
use std::fs::{read_dir, DirEntry};
use std::io::Error;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};


extern crate tokio;
use tokio::prelude::*;

extern crate futures;
use futures::future::{self, FutureExt, TryFutureExt};
use futures::stream::{self, StreamExt};


extern crate gdk_pixbuf;
use gdk_pixbuf::prelude::*;
use gdk_pixbuf::Pixbuf;

extern crate gio;
use gio::prelude::*;
use gio::Cancellable;

extern crate glib;
// use glib::prelude::*;

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
    selected: Arc<Mutex<bool>>,
    eventbox: EventBox,
}

impl LoadedImage {
    // fn new(path: PathBuf, height: i32) -> LoadedImage {
        // println!("{:?}", path);
        // let file = gio::File::new_for_path(path);
        // println!("{:?}", file);
        // file.read_async_future(glib::PRIORITY_DEFAULT)
        //     .map_err(|(_file, err)| {
        //         format!("Failed to open file: {}", err)
        //     })
        //     .and_then(move |(_file, stream)| {
        //         println!("Opened stream: {}", stream)
        //     }).await;
        // let pbuf = Pixbuf::new_from_stream_at_scale_async(
        //     file.read_async(0, gio::NONE_CANCELLABLE, ),
        //     -1, height, true, gio::NONE_CANCELLABLE,
        //     |a|{}
        // );
    // }
    // async fn new_async_future(path: Pathbuf, height: i32) -> Option<LoadedImage> {

    // }

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

        // Create new eventbox to track image selection clicks.
        let ebox = EventBox::new();
        let selected = Arc::new(Mutex::new(false));
        // let selected = Rc::new(RefCell::new(false));

        ebox.add(&img);

        // Configure click handler for image.
        ebox.connect_button_press_event(clone!(selected => move |w, e| {
            let widget_style = w.get_style_context();
            println!("clicked on: {:?} {:?}!", w, selected);
            match widget_style.has_class("selected") {
                true => {
                    widget_style.remove_class("selected");
                    *selected.lock().unwrap() = false;
                    // selected.replace(false);
                },
                false => {
                    widget_style.add_class("selected");
                    *selected.lock().unwrap() = true;
                    // selected.replace(true);
                },
            }
            Inhibit(false)
        }));

        Some(LoadedImage {
            img,
            pbuf,
            path,
            selected: selected,
            eventbox: ebox,
        })

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
    photos: Arc<Mutex<Vec<LoadedImage>>>,
}

impl EzrPhotoViewerApplication {
    pub fn run() -> Arc<Self> {
        let ea = EzrPhotoViewerApplication {
            photos: Arc::new(Mutex::new(Vec::new())),
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


fn pixbuf_future_from_dir<P: AsRef<Path>>(dir: P) {
    let file = gio::File::new_for_path(dir);
                    // .read_async_future(glib::PRIORITY_DEFAULT)
               
    println!("file: {:?}", file);

    // let file = block_on(file.read_async_future(glib::PRIORITY_DEFAULT));
    println!("file: {:?}", file);

    // 10
    // file.await
    // file.map_err()
    // tokio::run(file);
    // println!("{:?}", file);
}


#[derive(Clone)]
struct AppWindow {
    window: gtk::Window,
    container: gtk::Box,
}

impl AppWindow {
    fn new(main_app: Arc<EzrPhotoViewerApplication>) -> Arc<EzrPhotoViewerApplication> {
        // Main window.
        let window = gtk::Window::new(gtk::WindowType::Toplevel);

        // gtk::ScrolledWindow is the main container which the top level window will hold. It will hold the rest of the application.
        let scrolled_window = gtk::ScrolledWindow::new(NONE_ADJUSTMENT, NONE_ADJUSTMENT);
        // Add automatic scrolling for vertical only.
        scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled_window.set_border_width(0);

        // Add a hbox to contain all dynamically added photo rows.
        let hbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        scrolled_window.add(&hbox);

        // Add scrolled window to our main app window, set the app window size, and add a window size change callback.
        window.add(&scrolled_window);
        window.set_size_request(DEFAULT_WIDTH, DEFAULT_HEIGHT);
        window.show_all();

        // Add window resize callback
        window.connect_size_allocate(clone!(window => move |obj, rect| {
            println!("new allocation {:?}", rect);
        }));

        let app_window = Self {
            window: window,
            container: hbox,
        };

        Self::initialize_callbacks(&app_window);

        main_app
    }

    fn initialize_callbacks(app_window: &Self) {
        // app_window.connect_size_allocate(clone!(app_window => move |obj, rect| {
        //     println!("new allocation {:?}", rect);
        // }));
    }
}

// Default application window size.
const DEFAULT_HEIGHT: i32 = 1390;
const DEFAULT_WIDTH: i32 = 1250;

// Ratio of img height to total app height, e.g 5 is 5:1 ratio
const IMG_RATIO_TO_APP_HEIGHT: i32 = 7;


fn async_read_open() {

}

/// Validate that a std::fs::DirEntry is a valid file and within specified image limit.
fn validate_dir_entry(entry: &Result<DirEntry, Error>, limit: usize, count: usize) -> bool {
    if limit > 0 {
        if count >= limit {
            return false
        }
    }
    if let Err(_) = entry {
        return false
    }

    true
}

async fn async_test<P: AsRef<Path>>(path: P, loaded_images: Arc<Mutex<Vec<LoadedImage>>>, limit: usize) {

    // need to limit # of open files

    // let mut pixbufs: Vec<_> = Vec::new();
    let mut count = 0;

    let filenames: Vec<PathBuf> = read_dir(path)
                                .unwrap()
                                .filter_map(|e| {
                                    if validate_dir_entry(&e, limit, count) {
                                        count += 1;
                                        Some(e.unwrap().path())
                                    } else {
                                        None
                                    }
                                })
                                .collect();

    let mut pixbufs: Vec<Result<Pixbuf, ()>> = Vec::new();

    // Create and await futures in chunks of 50 filenames.
    for chunk in filenames.chunks(50) {

        let resolved = future::join_all(chunk.iter()
                .map(|p| {
                    gio::File::new_for_path(p)
                        .read_async_future(glib::PRIORITY_DEFAULT)
                        .map_err(|err| {println!("Error opening: {:?}", err)})
                        .and_then(|res| {
                            Pixbuf::new_from_stream_async_future(&res)
                                    .map_err(|p_err| { println!("Error creating pixbuf: {:?}", p_err); })
                        })
                })
            ).await;

        pixbufs.extend(resolved);
        
        // future::join_all(futures).await;
                    
        
    
    }
    println!("{:?}", pixbufs);
    println!("{:?}", pixbufs.len());
    // println!("{:?}", filenames);
    // for chunk in &read_dir(path).unwrap()
    //             // Apply filter condition to check if dir entry is valid, and if we are under max photo limit.
    //             .filter(|i| {
    //                 if limit > 0 {
    //                     let valid = match i {
    //                         Ok(i) => {
    //                             count += 1;
    //                             true
    //                         },
    //                         Err(_) => false,
    //                     };
    //                     let under_limit = limit > 0 && count <= limit;
    //                     valid && under_limit
    //                 } else { true }
    //             })
    //             // Can remove enumerate/inspect once not testing
    //             .enumerate()
    //             .inspect(|(i, e)| {
    //                 println!("{:?}", e)
    //             })
    //             // Create file (which will return a future, can probably move to it's own func after) for each dir entry.
    //             .map(|(i, e)| {
    //                 gio::File::new_for_path(e.unwrap().path())
    //                                 // Create future for file read.
    //                                 .read_async_future(glib::PRIORITY_DEFAULT)
    //                                 .map_err(move |err| { println!("err loading: {} {:?}", i, err )})
    //                                 .map_ok(move |res| {
    //                                     println!("loaded: {} {:?}", i, res);
    //                                     res
    //                                 })
    //                                 // Chain future to Pixbuf creation. Load pixbuf from file stream when completed.
    //                                 .and_then(move |res| {
    //                                     Pixbuf::new_from_stream_async_future(&res)
    //                                         .map_err(move |er| {println!("err creating pixbuf: {} {:?}", i, er)})
    //                                         .map_ok(move |re| {println!("created pixbuf: {} {:?}", i, re); re})
    //                                 })
    //             }).chunks(200) {
    //                 &pixbufs.extend(future::join_all(chunk.collect::<Vec<_>>()).await);
                    
    // }
    // println!("{:?}", pixbufs);
                // Collect futures.
                // .collect();
    
    // iter dir and create futures for each entry, then collect 
    // println!("{}", futures.len());
    println!("running..");
    // futures.chunk
    // println!("{:?}", futures);
    // let takes: Vec<usize> = futures.chunks(200).map(|chunk| chunk.len()).collect();

    // let futures = futures.into_iter();
    // for t in takes {
    //     &futures.take(t);
    // }
    // future::join_all(futures).await;
    // let file = gio::File::new_for_path(path)
    //                 .read_async_future(glib::PRIORITY_DEFAULT)
    //                 .map_err(|err| { println!("err loading: {:?}", err )})
    //                 .map_ok(|res| { println!("loaded: {:?}", res); res });
    // let file = file.await;
    // println!("{:?}", file);
    println!("ran?");
}



/// glib 
/// needs to either be 1 function with no return that takes a dir and arc mutex to house all loaded images,
/// or need to iter spawn local for each function which loads 1 photo path and adds it to arc mutex


#[tokio::main]
async fn main() {


    if let Ok(_) = gtk::init() {
        // load_images_from_dir("/home/cam/Downloads/desktop_walls", 0).await;, 100

        let loaded_images =  Arc::new(Mutex::new(Vec::new()));
        let li_clone = loaded_images.clone();
        let c = glib::MainContext::default();
        c.spawn_local(async_test("/home/cam/Downloads/desktop_walls", li_clone, 0));
        println!("spawned?");
        // assert_eq!(xx, Ok(9));
        // println!("ye {}", xx);
        // thread_load_images_from_dir("/home/cam/Downloads/desktop_walls", 0);
        // EzrPhotoViewerApplication::run();
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
