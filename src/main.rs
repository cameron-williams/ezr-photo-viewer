
use std::fs::read_dir;
use std::path::{Path, PathBuf};

use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;

extern crate gdk_pixbuf;
use gdk_pixbuf::Pixbuf;

extern crate gtk;
use gtk::prelude::*;
use gtk::{
    EventBox, Image, Layout, StyleContext, NONE_ADJUSTMENT,
};



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

// A struct representing an Image once it's been loaded from a path into a gtk::Image/Pixbuf
#[derive(Debug)]
struct LoadedImage {
    img: Image,
    pbuf: Pixbuf,
    path: PathBuf,
    drawn: Rc<RefCell<bool>>,
    selected: Rc<RefCell<bool>>,
    eventbox: EventBox,
}

impl LoadedImage {
    
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

        // Create new eventbox to track image selection clicks.
        let ebox = EventBox::new();

        // 2 refcell's to track image status
        let selected = Rc::new(RefCell::new(false));
        let drawn = Rc::new(RefCell::new(false));

        ebox.add(&img);

        // Add click handler for image.
        ebox.connect_button_press_event(clone!(selected => move |w, e| {
            let widget_style = w.get_style_context();
            println!("clicked on: {:?} {:?}!", w, selected);
            match widget_style.has_class("selected") {
                true => {
                    widget_style.remove_class("selected");
                    selected.replace(false);
                },
                false => {
                    widget_style.add_class("selected");
                    selected.replace(true);
                },
            }
            Inhibit(false)
        }));

        Some(LoadedImage {
            img,
            pbuf,
            path,
            selected,
            drawn,
            eventbox: ebox,
        })

    }

    fn width(&self) -> i32 {
        self.pbuf.get_width()
    }

    fn height(&self) -> i32 {
        self.pbuf.get_height()
    }
}


struct EzrPhotoViewerApplication {
    images: Rc<RefCell<Vec<LoadedImage>>>,
}

impl EzrPhotoViewerApplication {
    pub fn run() -> Arc<Self> {
        let ea = EzrPhotoViewerApplication {
            images: Rc::new(RefCell::new(Vec::new())),
        };
        AppWindow::new(Arc::new(ea))
    }
}

// Default application window size.
const DEFAULT_HEIGHT: i32 = 1390;
const DEFAULT_WIDTH: i32 = 1250;

// Ratio of img height to total app height, e.g 5 is 5:1 ratio
const IMG_RATIO_TO_APP_HEIGHT: i32 = 7;


fn load_images_for_path<P: AsRef<Path>>(path: P, limit: bool) -> Vec<LoadedImage> {
    // count to be removed
    let mut count = 0;
    read_dir(path)
        .unwrap()
        .filter_map(|e| {
            if let Err(_) = e {
                return None
            }
            // ( to be removed
            if limit && count > 2 {
                return None
            }
            count += 1;
            // to be removed )
            Some(e.unwrap())
        })
        .filter_map(|e| {
            // LoadedImage already returns a Result so no need to wrap it in Some/None
            println!("loading: {:?}", e.path());
            LoadedImage::new(e.path(), 200)
        })
        .collect()
}


#[derive(Clone)]
struct AppWindow {
    window: gtk::Window,
    container: gtk::Layout,
    dimensions: Rc<RefCell<(i32, i32)>>,
    application: Arc<EzrPhotoViewerApplication>,
}

impl AppWindow {
    fn new(main_app: Arc<EzrPhotoViewerApplication>) -> Arc<EzrPhotoViewerApplication> {

        // Main window.
        let window = gtk::Window::new(gtk::WindowType::Toplevel);


        // Base layout which will hold all the images.
        let layout = gtk::Layout::new(
            NONE_ADJUSTMENT,
            NONE_ADJUSTMENT
        );
        layout.set_size(
            DEFAULT_WIDTH as u32,
            DEFAULT_HEIGHT as u32
        );

        // Load CSS.
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_path("/home/cam/Programming/rust/ezr-photo-viewer/src/app.css").expect("failed loading app CSS");
        StyleContext::add_provider_for_screen(&window.get_screen().unwrap(), &css_provider, 1);

        // Load images from directory.
        println!("Reading images");
        main_app.images.replace(
            load_images_for_path("/home/cam/Downloads/desktop_walls", true)
        );
        
        
        println!("OK: Read {} images", main_app.images.borrow().len());

        // Add layout to our main app window, set the app window size, and add a window size change callback.
        window.add(&layout);
        window.set_size_request(DEFAULT_WIDTH, DEFAULT_HEIGHT);
        window.show_all();

        // set dimensions to default w/h
        let dimensions = Rc::new(RefCell::new((DEFAULT_WIDTH, DEFAULT_HEIGHT)));

        let app_window = Arc::new(Self {
            window: window,
            container: layout,
            dimensions,
            application: Arc::clone(&main_app),
        });

        Self::draw_photos(Arc::clone(&app_window));


        println!("Initializing callbacks");
        Self::initialize_callbacks(Arc::clone(&app_window));
    
        main_app
    }

    fn initialize_callbacks(app_window: Arc<Self>) {

        // Add callback for resizing photos on new window dimensions.
        app_window.window.connect_size_allocate(clone!(app_window => move |obj, rect| {
            let mut dimensions = app_window.dimensions.borrow_mut();
            if *dimensions != (rect.width, rect.height) {
                
                *dimensions = (rect.width, rect.height);
                // drop mutable reference once new value is set
                std::mem::drop(dimensions);

                Self::draw_photos(Arc::clone(&app_window));

            }

        }));

        println!("Initialized callbacks.")

    }

    // This function will draw/redraw all photos to the layout.
    fn draw_photos(app_window: Arc<Self>) {
        println!("drawing");
        let row_height = DEFAULT_HEIGHT / IMG_RATIO_TO_APP_HEIGHT;
        let max_width  = app_window.dimensions.borrow().0;
        println!("max width: {}, row height: {}", max_width, row_height);
        let row_spacing = 10;

        let mut current_row_index = 0;
        let mut current_row_width = 0;
        let mut current_row_items: Vec<&LoadedImage> = Vec::new();

        let images = app_window.application.images.borrow();

        for img in images.iter() {

            if (img.width() + current_row_width) >= max_width {
                Self::draw_to_layout_from_vec(
                    &app_window.container,
                    &current_row_items,
                    current_row_index,
                    row_height,
                    row_spacing
                );
                current_row_index += 1;
                current_row_width = 0;
                &current_row_items.clear();
            }

            &current_row_items.push(img);
            current_row_width += img.width();

        }

        if current_row_items.len() > 0 {
            Self::draw_to_layout_from_vec(
                &app_window.container,
                &current_row_items,
                current_row_index,
                row_height,
                row_spacing
            );
            current_row_index += 1;
        }
        app_window.container.set_size(
            max_width as u32,
            (current_row_index * row_height) as u32,
        );
        app_window.window.show_all();
    }

    fn draw_to_layout_from_vec(layout: &gtk::Layout, row: &Vec<&LoadedImage>, row_index: i32, row_height: i32, row_spacing: i32) {
        // Get max width and determine how much free space we have for the current row from that.
        println!("layout size: {} - {}", layout.get_size().0, row.iter()
                                .fold(0, |sum, i| {sum + i.width()}));
        let free_space = layout.get_size().0 as i32
                            - row.iter()
                                .fold(0, |sum, i| {sum + i.width()}) as i32;
        
        println!("free space: {}", free_space);

        // Set spacing.
        let spacing: i32 = free_space / (row.len() as i32 + 1);

        // Determine initial x/y positioning.
        let mut pos_x = spacing;
        let pos_y = (row_height * row_index) + row_spacing;

        

        // Put or move all images to their proper positions.
        for image in row {

            println!("x:{}, y:{}", pos_x, pos_y);    
            // Check if image is drawn already, if it is move it, otherwise put it and set drawn to true.
            let mut drawn = image.drawn.borrow_mut();
            match *drawn { 
                false => {
                    layout.put(&image.eventbox, pos_x, pos_y);
                    *drawn = true;
                },
                true => {
                    layout.move_(&image.eventbox, pos_x, pos_y);
                }
            }
            // Update the next placement's pos_x accordingly.
            pos_x += image.width() + spacing;
        }  

    }
}


fn main() {

    if let Ok(_) = gtk::init() {
        EzrPhotoViewerApplication::run();
        gtk::main();
    }
    
}
