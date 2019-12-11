use std::fs::read_dir;
use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

extern crate gdk_pixbuf;
use gdk_pixbuf::Pixbuf;

extern crate gtk;
use gtk::prelude::*;
use gtk::{EventBox, Image, StyleContext, NONE_ADJUSTMENT};

extern crate gdk;
use gdk::EventType::{ButtonPress, DoubleButtonPress};

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

// Default application window size.
const DEFAULT_HEIGHT: i32 = 1390;
const DEFAULT_WIDTH: i32 = 1250;

// Ratio of img height to total app height, e.g 5 is 5:1 ratio
const IMG_RATIO_TO_APP_HEIGHT: i32 = 7;

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

/// Represents an image that has been loaded from a path.
/// The actual widget that is added to the layout is the eventbox, which wraps the actual gtk::Image.
/// This eventbox handles the highlighting/selecting when clicking on images.
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
    /// Constructor function for an image which will be added to the main photo Layout.
    pub fn new(path: PathBuf, height: i32) -> Option<LoadedImage> {
        let pbuf = match Pixbuf::new_from_file_at_scale(&path, -1, height, true) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("err opening image {:?}", e);
                return None;
            }
        };
        let img = Image::new_from_pixbuf(Some(&pbuf));

        // Create new eventbox to track image selection clicks.
        let ebox = EventBox::new();
        ebox.add(&img);

        // 2 refcell's to track image statuses.
        let selected = Rc::new(RefCell::new(false));
        let drawn = Rc::new(RefCell::new(false));

        // Add image click handlers for single and double click.
        ebox.connect_button_press_event(clone!(selected, path => move |w, e| {
            match e.get_event_type() {
                ButtonPress => {
                    Self::toggle_selected(w, Rc::clone(&selected));
                },
                DoubleButtonPress => {
                    Self::handle_double_click(w, &path);
                }
                _ => {}

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

    /// Loads an image and creates it as a new toplevel window.
    /// Eventuall need to add something that limits the max window size.
    fn new_as_popup(path: &PathBuf) -> Result<gtk::Window, String> {
        let new_window = gtk::Window::new(gtk::WindowType::Toplevel);

        let img = match Pixbuf::new_from_file(path) {
            Ok(pixbuf) => gtk::Image::new_from_pixbuf(Some(&pixbuf)),
            Err(_) => {
                eprintln!("failed to open {:?} for new popup window", path);
                return Err(format!("failed to open {:?} for popup window", path));
            }
        };

        new_window.add(&img);

        Ok(new_window)
    }

    fn width(&self) -> i32 {
        self.pbuf.get_width()
    }

    /// When called will flip the selected status for specified EventBox.
    fn toggle_selected(w: &EventBox, selected: Rc<RefCell<bool>>) {
        let widget_style = w.get_style_context();
        println!("clicked on: {:?} {:?}!", w, selected);
        match widget_style.has_class("selected") {
            true => {
                widget_style.remove_class("selected");
                selected.replace(false);
            }
            false => {
                widget_style.add_class("selected");
                selected.replace(true);
            }
        }
    }

    /// Image double click handler. When an image is double
    /// clicked on the UI, pop open that image in a new window
    /// that has the same dimensions as the original photo.
    ///
    /// Likely will add more functionality to this later. Such as the ability to go to the next photo.
    fn handle_double_click(w: &EventBox, path: &PathBuf) {
        println!("double clicked image, need to pop it now");
        println!("{:?}\n{:?}", w, path);

        match Self::new_as_popup(path) {
            Ok(window) => window.show_all(),
            Err(err) => eprintln!("failed to create popup window: {:?}", err),
        }
    }
}

/// AppWindow represents the main TopLevel window which holds the photo layout.
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

        // Base layout which will hold all the images. Set the initial size to default.
        let layout = gtk::Layout::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        layout.set_size(DEFAULT_WIDTH as u32, DEFAULT_HEIGHT as u32);

        // ScrolledWindow holds the layout and allows it to be scrollable.
        let scrolled_window = gtk::ScrolledWindow::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        scrolled_window.add(&layout);

        // Load CSS and set the window style provider.
        let css_provider = gtk::CssProvider::new();
        css_provider
            .load_from_path("/home/cam/Programming/rust/ezr-photo-viewer/src/app.css")
            .expect("failed loading app CSS");
        StyleContext::add_provider_for_screen(&window.get_screen().unwrap(), &css_provider, 1);

        // Load images and set application images from directory.
        *main_app.images.borrow_mut() =
            load_images_for_path("/home/cam/Downloads/desktop_walls");

        // Add layout to our main app window, set the app window size request.
        window.add(&scrolled_window);
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
        Self::initialize_callbacks(Arc::clone(&app_window));
        main_app
    }

    fn initialize_callbacks(app_window: Arc<Self>) {
        // Add main window quit callback.
        app_window.window.connect_delete_event(|_w, _e| {
            gtk::main_quit();
            Inhibit(false)
        });

        // Add callback for resizing photos on new window dimensions.
        app_window
            .window
            .connect_size_allocate(clone!(app_window => move |_obj, rect| {
                let mut dimensions = app_window.dimensions.borrow_mut();
                if *dimensions != (rect.width, rect.height) {

                    *dimensions = (rect.width, rect.height);
                    // drop mutable reference once new value is set, since it's borrowed again in the draw_photos fn.
                    std::mem::drop(dimensions);

                    Self::draw_photos(Arc::clone(&app_window));
                }

            }));
    }

    // This function will draw/redraw all photos to the layout.
    fn draw_photos(app_window: Arc<Self>) {
        
        // Row height is determined by the default height/img ratio consts, as this
        // was what was used to initially load the images.
        let row_height = DEFAULT_HEIGHT / IMG_RATIO_TO_APP_HEIGHT;

        let max_width = app_window.dimensions.borrow().0;
        let row_spacing = 20; // could be a const instead(? not sure if this is the rusty way to do it)

        // current_row_index is used to determine the pos_y of placed images,
        // as well as the max height that needs to be allocated to the layout at the end of the draw function.
        let mut current_row_index = 0;

        // current_row_width is used to determine how many photos can be added per row.
        let mut current_row_width = 0;

        // tracks row items in association with current_row_width
        let mut current_row_items: Vec<&LoadedImage> = Vec::new();


        let images = app_window.application.images.borrow();
        for img in images.iter() {

            // Check if the current image will not fit in the current Vec<Images>
            // If it doesn't, draw the current Vec<Images> and clear it.
            if (img.width() + current_row_width) >= max_width {
                Self::draw_to_row_from_vec(
                    &app_window.container,
                    &current_row_items,
                    current_row_index,
                    row_height,
                    row_spacing,
                );
                current_row_index += 1;
                current_row_width = 0;
                &current_row_items.clear();
            }

            &current_row_items.push(img);
            current_row_width += img.width();
        }

        // Not always do all photos fit perfectly in the last row, so we might have an extra row with 1-2 photos in it.
        // check to see if there are any and if there are draw them as the last row.
        if current_row_items.len() > 0 {
            Self::draw_to_row_from_vec(
                &app_window.container,
                &current_row_items,
                current_row_index,
                row_height,
                row_spacing,
            );
            current_row_index += 1;
        }

        // Set our main layout size to match that which the images take up.
        app_window
            .container
            .set_size(max_width as u32, (current_row_index * row_height) as u32);

        app_window.window.show_all();
    }


    /// Takes a row (just a vec of loaded images), and using a given row index, row height, and row spacing
    /// draws it to the given layout, allowing equal space between each image as well as moving the image if
    /// it's already placed somewhere else.
    fn draw_to_row_from_vec(
        layout: &gtk::Layout,
        row: &Vec<&LoadedImage>,
        row_index: i32,
        row_height: i32,
        row_spacing: i32,
    ) {
        
        // Get the max width we have for the current row, and determine how much free space there is to
        // divide between images based on the total row already used width.
        let free_space =
            layout.get_size().0 as i32 - row.iter().fold(0, |sum, i| sum + i.width()) as i32;

        // Determine the spacing between images (total free space / number of images).
        let spacing: i32 = free_space / (row.len() as i32 + 1);

        // Determine initial x/y positioning.
        let mut pos_x = spacing;
        let pos_y = (row_height * row_index) + row_spacing;

        for image in row {
            
            // Check if image is drawn already, if it is move it, otherwise put it and set drawn to true.
            let mut drawn = image.drawn.borrow_mut();
            match *drawn {
                false => {
                    layout.put(&image.eventbox, pos_x, pos_y);
                    *drawn = true;
                }
                true => {
                    layout.move_(&image.eventbox, pos_x, pos_y);
                }
            }
            // Update the next placement's pos_x accordingly.
            pos_x += image.width() + spacing;
        }
    }
}

/// Takes an argument which is a Path, reads it, and returns a vec of LoadedImages created
/// from all the entries from that directory.
fn load_images_for_path<P: AsRef<Path>>(path: P) -> Vec<LoadedImage> {
    read_dir(path)
        .unwrap()
        .filter_map(|e| {
            if let Err(_) = e {
                return None;
            }
            Some(e.unwrap())
        })
        .filter_map(|e| {
            // LoadedImage already returns a Result so no need to wrap it in Some/None.
            // Use default height / img ratio to set the image height. This same value is used later to determine row heights.
            println!("loading: {:?}", e.path());
            LoadedImage::new(e.path(), DEFAULT_HEIGHT / IMG_RATIO_TO_APP_HEIGHT)
        })
        .collect()
}

fn main() {
    if let Ok(_) = gtk::init() {
        EzrPhotoViewerApplication::run();
        gtk::main();
    }
}
