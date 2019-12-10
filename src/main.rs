
use std::fs::{read_dir, DirEntry};
use std::io::Error;
use std::path::{Path, PathBuf};

use std::sync::{Arc, Mutex};

use std::rc::Rc;

extern crate gdk_pixbuf;
use gdk_pixbuf::prelude::*;
use gdk_pixbuf::Pixbuf;

extern crate gio;
use gio::prelude::*;


extern crate gtk;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, CssProvider, EventBox, FileChooserAction,
    FileChooserButton, FileChooserWidget, Grid, Image, Layout, Orientation, ScrolledWindow,
    StyleContext, NONE_ADJUSTMENT,
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
    // parent: Arc<Mutex<Option<ImageRow>>>,
    drawn: Arc<Mutex<bool>>,
    selected: Arc<Mutex<bool>>,
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
        let drawn = Arc::new(Mutex::new(false));

        // Create new eventbox to track image selection clicks.
        let ebox = EventBox::new();
        let selected = Arc::new(Mutex::new(false));

        ebox.add(&img);

        // Add click handler for image.
        ebox.connect_button_press_event(clone!(selected => move |w, e| {
            let widget_style = w.get_style_context();
            println!("clicked on: {:?} {:?}!", w, selected);
            match widget_style.has_class("selected") {
                true => {
                    widget_style.remove_class("selected");
                    *selected.lock().unwrap() = false;
                },
                false => {
                    widget_style.add_class("selected");
                    *selected.lock().unwrap() = true;
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

struct PGrid {
    layout: Layout,
    window: ScrolledWindow,
    max_height: i32,
    max_width: i32,
    row_spacing: i32,
    row_height: i32,
    last_rect: gtk::Rectangle,
    images: Vec<LoadedImage>, // might have to wrap in a Rc<RefCell<>>
}

impl PGrid {
    pub fn new(width: i32, height: i32) -> PGrid {
        let layout = Layout::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        let window = ScrolledWindow::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);

        let button = Button::new_with_label("Email Selected Photos");
        layout.put(&button, 100, 700);
        window.add(&layout);
        // Cloned widgets for use in window.connect_scroll_event()
        let button_clone = button.clone();
        let layout_clone = layout.clone();

        window.connect_scroll_event(move |s, e| {
            // println!("scrolled {:?} {:?}", s, e);
            // let adjustment = s.get_vadjustment().unwrap();
            // println!("{:?}", s.get_vadjustment().unwrap().get_value());
            let current_scroll_value = s.get_vadjustment().unwrap().get_value();
            layout_clone.move_(&button_clone, 400, 700 + current_scroll_value as i32);
            Inhibit(false)
        });

        PGrid {
            layout,
            window,
            max_height: height,
            max_width: width,
            row_spacing: 5,
            row_height: DEFAULT_HEIGHT / IMG_RATIO_TO_APP_HEIGHT,
            last_rect: gtk::Rectangle {
                x: 0,
                y: 0,
                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,
            },
            images: Vec::new(),
        }
    }

    // Sets the last rect to given gtk::rect.
    pub fn set_last_rect(&mut self, rect: &gtk::Rectangle) {
        self.last_rect = gtk::Rectangle {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
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
    // pub fn load_images_from_dir<P: AsRef<Path>>(&mut self, dir: P) {
    //     // let mut count = 0;
    //     for entry in read_dir(dir).unwrap() {
    //         // if count > 30 {
    //         //     continue
    //         // }
    //         // count += 1;
    //         match entry {
    //             Ok(p) => {
    //                 match LoadedImage::new(p.path(), self.max_height/IMG_RATIO_TO_APP_HEIGHT) {
    //                     Some(img) => self.images.push(img),
    //                     None => continue
    //                 }
    //                 println!("Loaded {:?}", p.path())
    //             },
    //             Err(_) => continue,
    //         }
    //     }
    // }

    pub fn get_selected_images(&self) -> Vec<&LoadedImage> {
        let mut selected: Vec<&LoadedImage> = Vec::new();

        for i in self.images.iter() {
            let style_context = i.eventbox.get_style_context();
            if style_context.has_class("selected") {
                &selected.push(&i);
            }
        }
        selected
    }

    // Redraws all held images in self.images to layout according to self.max height/width.
    pub fn redraw(&self, initialize: bool) {
        let mut current_row_index = 0;
        let mut current_row_width = 0;
        let mut current_row_items: Vec<&LoadedImage> = Vec::new();

        for image in self.images.iter() {
            let image_width = image.pbuf.get_width();

            // If image does not fit in the current row, draw the current row and start from a new one.
            if (image_width + current_row_width) >= self.max_width {
                self.draw_row_from_vec(&current_row_items, current_row_index, initialize);
                current_row_index += 1;
                current_row_width = 0;
                &current_row_items.clear();
            }

            &current_row_items.push(&image);
            current_row_width += image_width;
        }
        // Because rows don't always end with a max length we need to do 1 final check to see
        // if there are any images left that haven't been drawn, and draw them in that case.
        if current_row_items.len() > 0 {
            self.draw_row_from_vec(&current_row_items, current_row_index, initialize);
            current_row_index += 1;
            &current_row_items.clear();
        }

        // Set layout width/height. This is used to determine scrollbars by the ScrolledWindow.
        self.layout.set_size(
            self.max_width as u32,
            (current_row_index * self.row_height) as u32,
        );
    }

    // Takes a &Vec<&LoadedImage> and draws it to the layout, calculating proper spacing between images.
    pub fn draw_row_from_vec(&self, row: &Vec<&LoadedImage>, row_index: i32, initialize: bool) {
        // Determine how much free space we have for the current row. TODO:// Get rid of double .iter()?
        let mut free_space = self.max_width;
        for r in row.iter() {
            free_space -= r.pbuf.get_width();
        }

        // Determine spacing.
        let spacing = free_space / (row.len() as i32 + 1);

        let mut x = spacing;
        let y = (self.row_height * row_index) + self.row_spacing;

        // Iterate images and place or move them to their proper positions on the layout.
        for image in row.iter() {
            match initialize {
                true => {
                    self.layout.put(&image.eventbox, x, y);
                }
                false => self.layout.move_(&image.eventbox, x, y),
            }
            x += image.pbuf.get_width();
            x += spacing;
        }
    }
}

struct EzrPhotoViewerApplication {
    // images: Rc<Vec<LoadedImage>>,
    images: Arc<Mutex<Vec<LoadedImage>>>,
    // images: Arc<Mutex<Vec<Arc<LoadedImage>>>>,
}

impl EzrPhotoViewerApplication {
    pub fn run() -> Arc<Self> {
        let ea = EzrPhotoViewerApplication {
            // images: Rc::new(Vec::new());
            // images: Ar
            images: Arc::new(Mutex::new(Vec::new())),
        };
        AppWindow::new(Arc::new(ea))
    }
}

// Read and load images from specified path.
async fn load_images_from_dir<P: AsRef<Path>>(dir: P, limit: usize,) -> Arc<Mutex<Vec<LoadedImage>>> {
    // Init mutex<vec> to hold all LoadedImages
    let images = Arc::new(Mutex::new(Vec::new()));
    let mut count = 0;
    // let mut ft_images = Vec::new();
    
    // Iterate all image file paths, load the contents as a stream, and then create Pixbufs from them asynchronously.
    // read_dir(dir)
    //             .unwrap()
    //             .filter(|i| {
    //                 let valid = match i {
    //                         Ok(i) => true,
    //                         Err(_) => false,
    //                     };
    //                 let under_limit = limit > 0 && count <= limit;
    //                 valid && under_limit
    //             })
    //             .map(|i| {
    //                 gio::File::new()
    //             })
    //             ;

    // Iterate all dir entries and create LoadedImages futures from them, await all to finish loading.
    // let stream = stream::iter(read_dir(dir).unwrap().filter(|i| {
    //     if limit > 0 {
    //         let valid = match i {
    //         Ok(i) => {
    //             count += 1;
    //             true
    //         },
    //         Err(_) => false,
    //     };
    //     let under_limit = limit > 0 && count <= limit;
    //     valid && under_limit
    //     } else { true }
    // })).then(|x| async move { println!("loaded {:?}", x); LoadedImage::new(x.unwrap().path(), 100) }).collect::<Vec<_>>().await;

    // println!("{:?}", stream.len());
    // println!("{:?}", stream);
    println!("hello?");



    images
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
    // container: gtk::Box,
    container: gtk::Layout,
    dimensions: Arc<Mutex<(i32, i32)>>,
    // dimensions: Mutex<(i32, i32)>,
    application: Arc<EzrPhotoViewerApplication>,
}

impl AppWindow {
    fn new(main_app: Arc<EzrPhotoViewerApplication>) -> Arc<EzrPhotoViewerApplication> {

        // Main window.
        let window = gtk::Window::new(gtk::WindowType::Toplevel);

        let layout = gtk::Layout::new(
            NONE_ADJUSTMENT,
            NONE_ADJUSTMENT
        );

        // // gtk::ScrolledWindow is the main container which the top level window will hold. It will hold the rest of the application.
        // let scrolled_window = gtk::ScrolledWindow::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        // // Add automatic scrolling for vertical only.
        // scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        // scrolled_window.set_border_width(0);

        // // Add a hbox to contain all dynamically added photo rows.
        // let hbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // scrolled_window.add(&hbox);

        // Load CSS.
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_path("/home/cam/Programming/rust/ezr-photo-viewer/src/app.css").expect("failed loading app CSS");
        StyleContext::add_provider_for_screen(&window.get_screen().unwrap(), &css_provider, 1);

        // Load images from directory.
        println!("Reading images");
        *main_app.images.lock().unwrap() = load_images_for_path("/home/cam/Downloads/desktop_walls", true);
        
        println!("OK: Read images");

        // Add scrolled window to our main app window, set the app window size, and add a window size change callback.
        // window.add(&scrolled_window);
        window.add(&layout);

        window.set_size_request(DEFAULT_WIDTH, DEFAULT_HEIGHT);
        window.show_all();

        let app_window = Arc::new(Self {
            window: window,
            // container: hbox,
            container: layout,
            // dimensions: Mutex::new((0, 0)),
            dimensions: Arc::new(Mutex::new((0, 0))),
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

            if *app_window.dimensions.lock().unwrap() != (rect.width, rect.height) {
                
                *app_window.dimensions.lock().unwrap() = (rect.width, rect.height);
                
                Self::draw_photos(Arc::clone(&app_window));

            }

        }));

        println!("Initialized callbacks.")

    }

    // This function will draw/redraw all photos to the layout.
    fn draw_photos(app_window: Arc<Self>) {
        println!("drawing");
        for img in app_window.application.images.lock().unwrap().iter() {
            println!("{:?}", img);
            app_window.container.add(&img.eventbox);
        }
        app_window.window.show_all();
    }

    // fn _draw_photos(app_window: Arc<Self>) {
    //     println!("drawing!");
    //     let row_height = DEFAULT_HEIGHT / IMG_RATIO_TO_APP_HEIGHT;
    //     let max_width = app_window.dimensions.lock().unwrap().0;
    //     let row_spacing = 10;

    //     let mut current_row_index = 0;
    //     let mut current_row_width = 0;
    //     let mut current_row_items: Vec<&Arc<LoadedImage>> = Vec::new();

    //     let images = &*app_window.application.images.lock().unwrap();

    //     for image in images {

    //         // Check if image will fit in current row, if it won't draw current row and start from a new row.
    //         if (image.width() + current_row_width) >= max_width {
    //             Self::draw_to_layout_from_vec(
    //                 &app_window.container,
    //                 &current_row_items,
    //                 current_row_index,
    //                 row_height,
    //                 row_spacing
    //             );
    //             current_row_index += 1;
    //             current_row_width = 0;
    //             &current_row_items.clear();
    //         }

    //         // Push current image to row items and update the total current row width.
    //         &current_row_items.push(image);
    //         current_row_width += image.width();
    //     }

    //     // Rows don't always end with a max length, as such check if the current row has anything and draw it if it does.
    //     if current_row_items.len() > 0 {
    //         Self::draw_to_layout_from_vec(
    //             &app_window.container,
    //             &current_row_items,
    //             current_row_index,
    //             row_height,
    //             row_spacing
    //         );
    //         current_row_index += 1;
    //     }

    //     app_window.container.set_size(
    //         max_width as u32,
    //         (current_row_index * row_height) as u32,
    //     )

    // }

    // fn draw_to_layout_from_vec(layout: &gtk::Layout, row: &Vec<&Arc<LoadedImage>>, row_index: i32, row_height: i32, row_spacing: i32) {
    //     // Get max width and determine how much free space we have for the current row from that.
    //     let free_space = layout.get_size().0
    //                         - row.iter()
    //                             .fold(0, |sum, i| {sum + i.width()}) as u32;

    //     // Set spacing.
    //     let spacing: u32 = free_space / (row.len() as u32 + 1);

    //     // Determine initial x/y positioning.
    //     let mut pos_x = spacing as i32;
    //     let pos_y = (row_height * row_index) + row_spacing;

    //     // Put or move all images to their proper positions.
    //     for image in row {
    //         match *image.drawn.lock().unwrap() {
    //             true => {
    //                 layout.put(&image.eventbox, pos_x, pos_y);
    //             },
    //             false => {
    //                 layout.move_(&image.eventbox, pos_x, pos_y);
    //             }
    //         }
    //         // Update the next placement's pos_x accordingly.
    //         pos_x += image.width() + spacing as i32;
    //     }  

    // }


    // fn _draw_photos(app_window: Arc<Self>) {

    //     // Iterate any existing rows and remove images from any parent so they can be redrawn.
    //     app_window.container.foreach(|w| {
    //         println!("removing: {:?}", w);
    //         app_window.container.remove(w);
    //     });
        
    //     // Holds full ImageRows
    //     let mut completed_rows: Vec<ImageRow> = Vec::new();
    //     let mut current_row = ImageRow::new(DEFAULT_WIDTH);

    //     // Get the currently loaded images.
    //     let images = &*app_window.application.images.lock().unwrap();
    //     for img in images {
    //         println!("iter an image to try and make it");
            
    //         // Add image to current row, or if it doesn't fit create a new row and add it to that row.
    //         match current_row.try_push_image(Arc::clone(img)) {
    //             Some(i) => {
    //                 completed_rows.push(current_row);
    //                 current_row = ImageRow::new(DEFAULT_WIDTH);
    //                 current_row.try_push_image(i);
    //             },
    //             None => {println!("added image to a row")}
    //         }
    //     }

    //     // Check if there is a leftover row that hasn't been added to the completed rows vec yet.
    //     if *current_row.current_width.lock().unwrap() > 0 {
    //         completed_rows.push(current_row);
    //     }
        
    //     // Build rows and add them to the main application container.
    //     for row in completed_rows {
    //         println!("adding completed row");
    //         app_window.container.pack_start(
    //             &row.build(),
    //             false,
    //             false,
    //             5
    //         );
    //         println!("added completed row");
    //     }
    //     app_window.container.show_all();
    //     println!("{:?}", images.len());
        
    // }
}

/// ImageRow represents a future row of images that will be constructed 
/// and placed onto the application window.
#[derive(Debug)]
struct ImageRow {
    current_width: Mutex<i32>,
    max_width: i32,
    images: Arc<Mutex<Vec<Arc<LoadedImage>>>>,
}

impl ImageRow {

    fn new(max_width: i32) -> ImageRow {
        ImageRow {
            current_width: Mutex::new(0),
            images: Arc::new(Mutex::new(Vec::new())),
            max_width,
        }
    }

    // Consumes the ImageRow to build a gtk::Box with the images added.
    fn build(self) -> gtk::Box {
        let row = gtk::Box::new(Orientation::Horizontal, 10);
        row.set_homogeneous(false);

        for img in &*self.images.lock().unwrap() {
            
            row.pack_start(
                &img.eventbox,
                true,
                false,
                10,
            );
        }

        row
    }

    /// Attempts to add given LoadedImage to current row.
    /// If the row can fit the image, consume the image ref and add it to the images vec, returning None.
    /// If the row cannot fit the image will return the image wrapped as Some(img)
    fn try_push_image(&self, img: Arc<LoadedImage>) -> Option<Arc<LoadedImage>> {
        let mut current_width = self.current_width.lock().unwrap();

        // TODO need to add calculation to include row padding when checking if it can fit
        if *current_width + img.width() <= self.max_width {

            // Update width to include the newly added image.
            *current_width += img.width();

            // Push image to current row's vec.
            let mut images = self.images.lock().unwrap();
            images.push(img);
            None

        } else { Some(img)}
    }

}

fn main() {

    if let Ok(_) = gtk::init() {

        EzrPhotoViewerApplication::run();
        gtk::main();
    }
    // // Create application.
    // let application = Application::new(Some("com.github.ezr-photo-viewer"), Default::default())
    //     .expect("failed to initialize GTK application");

    // application.connect_activate(|app| {
    //     // Create application window.
    //     let window = ApplicationWindow::new(app);

    //     // Set default title and size.
    //     window.set_title("First GTK+ Program");
    //     window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);
    //     window.set_show_menubar(true);

    //     // let global_layout = Layout::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);

    //     // Add "photo selected" css provider/context
    //     let css_provider = CssProvider::new();
    //     css_provider.load_from_path("/home/cam/Programming/rust/ezr-photo-viewer/src/app.css");

    //     StyleContext::add_provider_for_screen(&window.get_screen().unwrap(), &css_provider, 1);

    //     // Initialize PhotoGrid in an Rc/RefCell
    //     let pg = Rc::new(RefCell::new(PGrid::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)));

    //     // Initial load/place of photos.
    //     pg.borrow_mut().load_images_from_dir("/home/cam/Downloads/desktop_walls");
    //     pg.borrow().redraw(true);

    //     // Add the PhotoGrid to the main app window.
    //     window.add(&pg.borrow().window);
    //     // global_layout.add(&pg.borrow().window);

    //     // let chooser = FileChooserButton::new("Select Photos Directory", FileChooserAction::SelectFolder);
    //     // window.add(&chooser);
    //     // window.add(&global_layout);

    //     // Add allocation change (window size change) callback.
    //     pg.borrow().window.connect_size_allocate(clone!(pg => move |obj, rect| {

    //         // Skip running resize operations if the new allocation is the same as the old one.
    //         if !pg.borrow_mut().check_allocation_change(&rect) {
    //             return
    //         }

    //         println!("new allocation {:?}", rect);

    //         // Set new max width/height according to new allocation.
    //         pg.borrow_mut().max_height = rect.height;
    //         pg.borrow_mut().max_width = rect.width;

    //         // Redraw photos.
    //         pg.borrow().redraw(false);

    //     }));

    //     // Draw all objects on window.
    //     window.show_all();

    // });

    // application.run(&[]);
}
