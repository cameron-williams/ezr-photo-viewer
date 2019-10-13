extern crate gtk;
extern crate gio;
extern crate gdk_pixbuf;


use std::rc::Rc;
use std::cell::RefCell;
use std::fs::read_dir;

use gtk::prelude::*;
use gio::prelude::*;
use gdk_pixbuf::prelude::*;

use std::path::Path;
use gtk::{Application, ApplicationWindow, ScrolledWindow, Button, EventBox, Grid, GridBuilder, PositionType, Image, Orientation, Layout, LayoutBuilder, ResizeMode, NONE_ADJUSTMENT, Viewport, StyleContext, CssProvider};
use gdk_pixbuf::{Pixbuf, InterpType};

const FILE_PATH: &str =  "/home/cam/Downloads/";
const FILE_NAMES: [&str; 5] = ["stock1.jpg", "stock2.jpg", "stock3.jpg", "stock4.jpg", "stock5.jpg"];


// GTK-RS recommended clone marco
macro_rules! clone {
    (@param _) => ( _ );
    (@param $x:ident) => ( $x );
    ($($n:ident),+ => move || $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move || $body
        }
    );
    ($($n:ident),+ => move |$($p:tt),+| $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move |$(clone!(@param $p),)+| $body
        }
    );
}

// Holds a loaded local image as gtk::Pixbuf and gtk::Image.
#[derive(Debug)]
struct LoadedImage {
    img: Image,
    pbuf: Pixbuf,
}

impl LoadedImage {

    // Creates a new LoadedImage, which stores a local image as both Image/Pixbuf
    // Specify a max height and the width will auto scale for the aspect ratio.
    pub fn new<P: AsRef<Path>>(path: P, height: i32) -> Option<LoadedImage> {
        let pbuf = match Pixbuf::new_from_file_at_scale(&path, -1, height, true) {
            Ok(p) => p,
            Err(e) => {
                println!("err opening image {:?}", e);
                return None
            },
        };

        let img = Image::new_from_pixbuf(Some(&pbuf));

        Some(LoadedImage {
            img,
            pbuf
        })

    }

}


struct PhotoGrid {
    max_width: i32,
    max_height: i32,
    last_rect: gtk::Rectangle,
    window: ScrolledWindow,
    grid: Grid,
    photos: Vec<LoadedImage>,
}

impl PhotoGrid {

    // Create new instance of PhotoGrid with default params.
    pub fn new() -> PhotoGrid {
        let window = ScrolledWindow::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        let grid = Grid::new();
        grid.set_column_homogeneous(false);
        // grid.set_row_homogeneous(true);
        grid.set_row_spacing(10);
        window.add(&grid);
        PhotoGrid {
            max_height: DEFAULT_HEIGHT,
            max_width: DEFAULT_WIDTH,
            last_rect: gtk::Rectangle {x: 0, y: 0, width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT},
            window,
            grid,
            photos: Vec::new(),
        }
    }

    // Sets the last rect to given gtk::rect.
    pub fn set_last_rect(&mut self, rect: &gtk::Rectangle) {
        self.last_rect = gtk::Rectangle {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height
        }
    }

    // Checks if there has been an allocation change.
    pub fn check_allocation_change(&mut self, new_rect: &gtk::Rectangle) -> bool {
        let mut is_changed = false;
        if self.last_rect.width != new_rect.width && self.last_rect.height != new_rect.height {
            is_changed = true;
            self.set_last_rect(new_rect);
        }
        is_changed
    }

    // Testing function. Loads all local test photos n times.
    pub fn load_photos(&mut self, n: i32) {
        for _ in 1..n {
            for fname in FILE_NAMES.iter() {
                let mut image_path = String::from(FILE_PATH);
                image_path.push_str(fname);
                match LoadedImage::new(image_path, self.max_height/IMG_RATIO_TO_APP_HEIGHT) {
                    Some(img) => self.photos.push(img),
                    None => continue
                }
            }
        }
    }

    pub fn load_images_from_dir<P: AsRef<Path>>(&mut self, dir: P) {
        for entry in read_dir(dir).unwrap() {
            match entry {
                Ok(p) => {
                    match LoadedImage::new(p.path(), self.max_height/IMG_RATIO_TO_APP_HEIGHT) {
                        Some(img) => self.photos.push(img),
                        None => continue
                    }
                    println!("Loaded {:?}", p.path())
                },
                Err(_) => continue,
            }
        }
    }

    // Places photos on the grid.
    pub fn place_photos(&mut self) {

        // Clear all grid rows.
        for c in self.grid.get_children() {
            self.grid.remove(&c);
        }

        // Tracks current row width.
        let mut row_width = 0;

        // Create new Box row which will hold the images.
        let mut row = gtk::Box::new(Orientation::Horizontal, 0);
        row.set_homogeneous(false);

        // Holds the completed Box rows.
        let mut rows: Vec<gtk::Box> = Vec::new();
        
        // Will hold the int widths of all items in current box, for figuring out spacing once row is full.
        let mut row_widths: Vec<i32> = Vec::new();

        // Iterate LoadedImages and add them into box rows.        
        for p in self.photos.iter() {
            
            // Put LoadedImage inside Eventbox.
            let widget = EventBox::new();
            widget.add(&p.img);

            // Add click handler for Eventbox.
            widget.connect_button_press_event(|w, e| {
                // w.set_border_width(3);
                println!("clicked: {:?} {:?}!", w, e);
                Inhibit(false)
            });

            let p_width = p.pbuf.get_width();

            // Image fits on current row.
            if p_width + row_width < self.max_width {
                // Record row width updates and push the image to the current row. 
                row_widths.push(p_width);
                row_width += p_width;
                // &row.pack_end(&p.img, false, false, 0);
                &row.pack_end(&widget, false, false, 0);

            // Image does not fit on current row.
            } else {
                   
                // Before we push the row as completed, we need to add spacers where we can
                // depending on how much free space there is in the row.
                let used_width: i32 = row_widths.iter().sum();
                let spacing_amount = (self.max_width - used_width) / (row_widths.len() as i32);
                row.set_spacing(spacing_amount);
                rows.push(row);

                // Create new box for new row, and clear previous row's row_widths.
                row = gtk::Box::new(Orientation::Horizontal, 0);
                row_widths = Vec::new();

                // Push the new row width and pack the row to the new box.
                row_widths.push(p_width);
                row_width = p_width;

                // Wrap picture in an event box then push to Box row.
                // row.pack_end(&p.img, false, false, 0)
                row.pack_end(&widget, false, false, 0)
                
            }
            
        }
        
        // iterate rows and add them to the grid at their proper row slot.
        for (i, r) in rows.iter().rev().enumerate() {
            self.grid.attach(r, 0, i as i32, 1, 1);
        }
        
    }


}

const DEFAULT_HEIGHT: i32 = 1390;
const DEFAULT_WIDTH: i32 = 1250;
// Ratio of img height to total app height, e.g 5 is 5:1 ratio
const IMG_RATIO_TO_APP_HEIGHT: i32 = 7;


fn main() {
    // Create application.
    let application = Application::new(Some("com.github.ezr-photo-viewer"), Default::default())
        .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        // Create application window.
        let window = ApplicationWindow::new(app);

        // Set default title and size.
        window.set_title("First GTK+ Program");
        window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);

        // Add "photo selected" css provider/context
        let css_provider = CssProvider::new();
        css_provider.load_from_path("/home/cam/Programming/rust/ezr-photo-viewer/src/app.css");

        StyleContext::add_provider_for_screen(&window.get_screen().unwrap(), &css_provider, 1);

        // Initialize PhotoGrid in an Rc/RefCell
        let pg = Rc::new(RefCell::new(PhotoGrid::new()));

        // Initial load/place of photos.
        // pg.borrow_mut().load_images_from_dir("/home/cam/Downloads/desktop_walls");
        pg.borrow_mut().load_photos(5);
        pg.borrow_mut().place_photos();

        pg.borrow().window.set_redraw_on_allocate(true);

        // Add the PhotoGrid to the main app window.
        window.add(&pg.borrow().window);
        
        // Add allocation change (window size change) callback.
        pg.borrow().window.connect_size_allocate(clone!(pg => move |obj, rect| {

            // Skip running resize operations if the new allocation is the same as the old one.
            if !pg.borrow_mut().check_allocation_change(&rect) {
                return
            }

            println!("new allocation {:?}", rect);

            // Set new max width/height according to new allocation.
            pg.borrow_mut().max_height = rect.height;
            pg.borrow_mut().max_width = rect.width;

            // Re-place the photos on the grid.
            pg.borrow_mut().place_photos();

            // Ensure all new placements are shown.
            pg.borrow().window.show_all();
        }));

        // Draw all objects on window.
        window.show_all();

    });

    application.run(&[]);
}
