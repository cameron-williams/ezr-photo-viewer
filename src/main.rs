extern crate gtk;
extern crate gio;
extern crate gdk_pixbuf;


use std::rc::Rc;
use std::cell::RefCell;
use std::fs::read_dir;
use std::collections::HashSet;

use gtk::prelude::*;
use gio::prelude::*;
use gdk_pixbuf::prelude::*;

use std::path::{Path, PathBuf};
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
#[derive(Clone)]
struct LoadedImage {
    img: Image,
    pbuf: Pixbuf,
    path: PathBuf,
    selected: Rc<RefCell<bool>>,
    eventbox: EventBox, 
}

impl LoadedImage {

    // Creates a new LoadedImage, which stores a local image as both Image/Pixbuf
    // Specify a max height and the width will auto scale for the aspect ratio.
    // Also holds the original path as well as the Eventbox which contains the image.
    pub fn new(path: PathBuf, height: i32) -> Option<LoadedImage> {
        let pbuf = match Pixbuf::new_from_file_at_scale(&path, -1, height, true) {
            Ok(p) => p,
            Err(e) => {
                println!("err opening image {:?}", e);
                return None
            },
        };

        // Create image from Pixbuf.
        let img = Image::new_from_pixbuf(Some(&pbuf));


        // let (ebox, status) = LoadedImage::new_event_box(&false);

        let ebox = EventBox::new();
        let selected_status = Rc::new(RefCell::new(false));

        ebox.add(&img);

        // Configure click handler for image.
        ebox.connect_button_press_event(clone!(selected_status => move |w, e| {
            let widget_style = w.get_style_context();
            println!("clicked on: {:?} {:?}!", w, selected_status);   
            match widget_style.has_class("selected") {
                true => {
                    widget_style.remove_class("selected");
                    selected_status.replace(false);
                },
                false => {
                    widget_style.add_class("selected");
                    selected_status.replace(true);
                },
            }
            Inhibit(false)
        }));

        
        Some(LoadedImage {
            img,
            pbuf,
            path,
            selected: selected_status,
            eventbox: ebox,
        })

    }

    // // Gets a new event box for current loadedimage.
    // pub fn get_new_event_box(&mut self) {
    //     let (ebox, status) = LoadedImage::new_event_box(&self.selected.borrow());
    //     ebox.add(&self.img);
    //     self.eventbox = ebox;
    //     self.selected = status;
    // }

    // // Creates a new Eventbox with a RefCell status.
    // pub fn new_event_box(selected: &bool) -> (EventBox, Rc<RefCell<bool>>) {
    //     let ebox = EventBox::new();
    //     let selected_status = Rc::new(RefCell::new(selected.to_owned()));

    //     // Configure click handler for image.
    //     ebox.connect_button_press_event(clone!(selected_status => move |w, e| {
    //         let widget_style = w.get_style_context();
    //         println!("clicked on: {:?} {:?}!", w, selected_status);   
    //         match widget_style.has_class("selected") {
    //             true => {
    //                 widget_style.remove_class("selected");
    //                 selected_status.replace(false);
    //             },
    //             false => {
    //                 widget_style.add_class("selected");
    //                 selected_status.replace(true);
    //             },
    //         }
    //         Inhibit(false)
    //     }));
    //     (ebox, selected_status)
    // }

}


struct PGrid {
    layout: Layout,
    window: ScrolledWindow,
    max_height: i32,
    max_width: i32,
    row_spacing: i32,
    row_height: i32,
    last_rect: gtk::Rectangle,
    images: Vec<LoadedImage>
}

impl PGrid {
    pub fn new() -> PGrid {
        // might need to switch to a real vertical adjustment
        let layout = Layout::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        let window = ScrolledWindow::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);

        window.add(&layout);

        PGrid {
            layout,
            window,
            max_height: DEFAULT_HEIGHT,
            max_width: DEFAULT_WIDTH,
            row_spacing: 5,
            row_height: DEFAULT_HEIGHT / IMG_RATIO_TO_APP_HEIGHT,
            last_rect: gtk::Rectangle {x: 0, y: 0, width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT},
            images: Vec::new(),
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


    // Read and load images from specified path.
    pub fn load_images_from_dir<P: AsRef<Path>>(&mut self, dir: P) {
        for entry in read_dir(dir).unwrap() {
            match entry {
                Ok(p) => {
                    match LoadedImage::new(p.path(), self.max_height/IMG_RATIO_TO_APP_HEIGHT) {
                        Some(img) => self.images.push(img),
                        None => continue
                    }
                    println!("Loaded {:?}", p.path())
                },
                Err(_) => continue,
            }
        }
    }

    pub fn redraw(&self, already_initialized: bool) {
        // act as if layout is empty.
        let mut current_row_index = 0;
        let mut current_row_width = 0;
        let mut current_row_items: Vec<&LoadedImage> = Vec::new();

        // Iterate each image.
        for image in self.images.iter() {
            
            let image_width = image.pbuf.get_width();

            // If the image fits in current row, add it.
            if (image_width + current_row_width) < self.max_width {
                &current_row_items.push(&image);
                current_row_width += image_width;

            // If the image does not fit in the current row, draw previous row and start with a new one.
            } else {
                
                // Draw current row and increment row counter.
                self.draw_row_from_vec(&current_row_items, current_row_index, already_initialized);
                current_row_index += 1;

                // Empty current row/width counter, and add the next image.
                current_row_items.clear();
                current_row_items.push(&image);
                current_row_width = image_width;
            }
        }

        println!("{:?}", (current_row_index * self.row_height));
        self.layout.set_size(self.max_width as u32, (current_row_index * self.row_height) as u32);
        println!("{:?}", self.layout.get_size());
    }

    pub fn draw_row_from_vec(&self, row: &Vec<&LoadedImage>, row_index: i32, already_initialized: bool) {
        
        // Get number of images.
        let n = row.len(); // 3

        // get amount of free width to divide.
        let mut free_space= self.max_width; // 300
        for r in row.iter() {
            free_space -= r.pbuf.get_width();
        }

        // figure out spacing amount after each image placement
        let spacing = free_space/(n as i32 +1); // 75
        let mut x = spacing;
        let y = (self.row_height * row_index) + self.row_spacing;

        for image in row.iter() {
            // Place image.
            match already_initialized {
                false => {
                    self.layout.put(
                        &image.eventbox,
                        x,
                        y,
                    );
                },
                true => {
                    self.layout.move_(
                        &image.eventbox,
                        x,
                        y,
                    )
                }
            }
            x += image.pbuf.get_width();
            x += spacing;
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
        let pg = Rc::new(RefCell::new(PGrid::new()));

        // Initial load/place of photos.
        pg.borrow_mut().load_images_from_dir("/home/cam/Downloads/desktop_walls");
        pg.borrow().redraw(false);

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

            // Redraw photos.
            pg.borrow().redraw(true);

        }));

        // Draw all objects on window.
        window.show_all();

    });

    application.run(&[]);
}
