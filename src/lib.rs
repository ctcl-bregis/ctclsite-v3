// ctclsite-rust - CTCL 2020-2024
// File: src/lib.rs
// Purpose: Module import and commonly used functions
// Created: November 28, 2022
// Modified: September 20, 2024

//use minifier::js;
//use minify_html::{minify, Cfg};
use comrak::{markdown_to_html, Options};
use image::{Rgb, RgbImage};
use indexmap::IndexMap;
use log::{error, info, warn};
use serde_json::value::Value;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Error, ErrorKind, Read, Write};
use std::path::Path;
use std::result::Result;
use tera::{Context, Tera};
use walkdir::WalkDir;

pub mod themes;
pub mod logger;
pub mod page;

pub use themes::*;
pub use logger::*;
pub use page::*;

// To-Do: This file is long, consider splitting some code into modules

// ----------------------------
// serde defaults

pub fn emptystringindexmap() -> IndexMap<String, String> {
    let map: IndexMap<String, String> = IndexMap::new();
    map
}

pub fn emptypagehashmap() -> HashMap<String, Page> {
    let map: HashMap<String, Page> = HashMap::new();
    map
}

pub fn emptythemehashmap() -> HashMap<String, Theme> {
    let map: HashMap<String, Theme> = HashMap::new();
    map
}

pub fn emptyfonthashmap() -> HashMap<String, FontFamily> {
    let map: HashMap<String, FontFamily> = HashMap::new();
    map
}

pub fn emptynavbarlinkhashmap() -> HashMap<String, NavbarLink> {
    let map: HashMap<String, NavbarLink> = HashMap::new();
    map
}

pub fn emptystring() -> String {
    String::new()
}

pub fn emptystringvec() -> Vec<String> {
    let vec: Vec<String> = Vec::new();
    vec
}

pub fn emptyusizevec() -> Vec<usize> {
    let vec: Vec<usize> = Vec::new();
    vec
}

pub fn emptytripleu8() -> [u8; 3] {
    let vec: [u8; 3] = [0u8; 3];
    vec
}

pub fn defaultfalse() -> bool {
    false
}

pub fn defaulttrue() -> bool {
    true
}

// ----------------------------

#[derive(Deserialize, Serialize)]
pub struct NavbarLink {
    title: String,
    link: String
}


#[derive(Deserialize, Serialize)]
pub enum ExtensionFileType {
    #[serde(alias = "binary")]
    Binary,
    // Anything with the "config" type shall not be copied to "static/"
    #[serde(alias = "config")]
    Config,
    #[serde(alias = "image")]
    Image,
    #[serde(alias = "pdf")]
    Pdf,
    #[serde(alias = "text")]
    Text,
    #[serde(alias = "video")]
    Video
}

#[derive(Deserialize, Serialize)]
pub struct SiteConfig {
    // IP to bind to, usually either "0.0.0.0" or "127.0.0.1"
    pub bindip: String,
    // Port to bind to
    pub bindport: u16,
    // Website domain, for example: "ctcl.lgbt". Currently used for the "link" meta tag.
    pub sitedomain: String,
    // Directory to scan for font directories
    pub fontdir: String,
    // Directory to scan for JavaScript files
    pub jsdir: String,
    // Directory to scan for page directories
    pub pagedir: String,
    // Directory to scan for globally-used static files
    pub staticdir: String,
    // Directory to scan for theme directories
    pub themedir: String,
    // Directory to scan for HTML templates
    pub templatedir: String,
    // Theme to default to if a specific theme is not defined. It must refer to an existing theme.
    pub defaulttheme: String,
    // How many "workers" to be deployed by Actix Web
    pub cpus: usize,
    // Map of links to redirect to
    pub redirects: HashMap<String, String>,
    // Links to make available in the navbar
    pub navbar: Vec<NavbarLink>,
    // Log configuration data 
    pub log: LogConfig,
    // Exists solely for debugging purposes. It should be set to "true" in production.
    pub minimizehtml: bool,
    // Definition of file types by file extension, used by collectstatic to determine what files to copy and may be used for the upcoming file viewer feature
    pub filetypes: HashMap<String, ExtensionFileType>,
    // Optional: Any extra parameters defined in config.json to be available in Lysine/Tera CSS templates
    pub themevars: Option<HashMap<String, Value>>,
    // Optional: Any extra parameters defined in config.json to be available in HTML templates
    pub uservars: Option<HashMap<String, Value>>,
    // Skip since pages, themes and fonts should not be defined in config.json
    #[serde(skip_deserializing, default = "emptypagehashmap")]
    pub pages: HashMap<String, Page>,
    #[serde(skip_deserializing, default = "emptythemehashmap")]
    pub themes: HashMap<String, Theme>,
    #[serde(skip_deserializing, default = "emptyfonthashmap")]
    pub fonts: HashMap<String, FontFamily>
}

// Partial config that only has fields for things required to start the webserver to avoid loading all of the pages twice
#[derive(Deserialize, Serialize)]
pub struct PartialSiteConfig {
    pub bindip: String,
    pub bindport: u16,
    pub cpus: usize
}

pub fn mkfavicons(themes: &HashMap<String, Theme>) -> Result<(), Error> {
    // At this point, static/ should exist
    mkdir("static/favicons/")?;

    for (key, value) in themes {
        // It is unlikely that default favicons would change so to reduce build time and disk writes, generation is skipped if the favicon already exists.
        if !fs::exists("static/favicons/default_{key}.ico").unwrap() {
            let mut image = RgbImage::new(16, 16);
            for x in 0..16 {
                for y in 0..16 {
                    let mut bytes = [0u8; 3];
                    hex::decode_to_slice(value.color.replace('#', ""), &mut bytes as &mut [u8]).unwrap();
                    image.put_pixel(x, y, Rgb(bytes));
                }
            }
            image.save(format!("static/favicons/default_{key}.ico")).unwrap_or_else(|_| panic!("Error while saving file default_{key}.ico"))
        }
    }

    Ok(())
}

pub fn buildjs(sitecfg: &SiteConfig) -> Result<(), Error> {
    mkdir("static/js/")?;

    match fs::read_dir(&sitecfg.jsdir) {
        Ok(d) => {
            for entry in d {
                match entry {
                    Ok(rd) => match fs::copy(rd.path(), format!("static/{}", rd.path().to_string_lossy())) {
                        Ok(_) => (),
                        Err(ce) => return Err(Error::new(ErrorKind::Other, format!("{ce}")))
                    }
                    Err(re) => return Err(Error::new(ErrorKind::Other, format!("{re}")))
                }
            }
        },
        Err(e) => return Err(Error::new(ErrorKind::Other, format!("{e}")))
    }

    Ok(())
}

pub fn collectstatic(sitecfg: &SiteConfig) -> Result<(), Error> {
    mkdir("static/pages/")?;

    for entry in WalkDir::new(&sitecfg.pagedir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_dir() {
            match entry.path().to_string_lossy().strip_prefix(&sitecfg.pagedir) {
                Some(p) => fs::create_dir_all(format!("static/pages/{}", p))?,
                None => fs::create_dir_all(format!("static/pages/{}", entry.path().to_string_lossy()))?
            }
        }
        
        if entry.path().is_file() {
            match entry.path().extension() {
                Some(fp) => match sitecfg.filetypes.get(&fp.to_string_lossy().into_owned()) {
                    Some(f) => match f {
                        ExtensionFileType::Config => continue,
                        _ => match entry.path().to_string_lossy().strip_prefix(&sitecfg.pagedir) {
                            Some(p) => fs::copy(entry.path(), format!("static/pages/{}", p)).unwrap(),
                            None => fs::copy(entry.path(), format!("static/pages/{}", entry.path().to_string_lossy())).unwrap()
                        }
                    }
                    None => continue
                },
                None => continue
            };
        };
    }

    match fs::read_dir(&sitecfg.staticdir) {
        Ok(d) => {
            for entry in d {
                match entry {
                    Ok(rd) => match fs::copy(rd.path(), format!("static/{}", rd.path().to_string_lossy().strip_prefix(&sitecfg.staticdir).unwrap())) {
                        Ok(_) => (),
                        Err(ce) => return Err(Error::new(ErrorKind::Other, format!("collectstatic failed to copy {} to static/{}, {}", rd.path().to_string_lossy(), rd.path().to_string_lossy(), ce)))
                    }
                    Err(re) => return Err(Error::new(ErrorKind::Other, format!("collectstatic: {re}")))
                }
            }
        },
        Err(e) => return Err(Error::new(ErrorKind::Other, format!("collectstatic: {e}")))
    }

    Ok(())
}

pub fn mkdir(path: &str) -> Result<(), Error> {
    match std::fs::create_dir(path) {
        Err(e) => match e.kind() {
            ErrorKind::AlreadyExists => {
                info!("loadfonts: directory {path} already exists, continuing");
                Ok(())
            }
            _ => {
                Err(Error::new(ErrorKind::Other, format!("Error creating directory {path}: {e}")))
            }
        }
        Ok(_) => {
            info!("{path} directory created");
            Ok(())
        }
    }
}

pub fn read_file<T: AsRef<Path>>(path: T) -> Result<String, Error> {
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => return Err(Error::new(ErrorKind::NotFound, format!("File not found: {}", path.as_ref().to_string_lossy()))),
            // Change from v1: do not panic; return an error instead
            _ => return Err(Error::new(ErrorKind::Other, format!("Error reading from file {}: {e}", path.as_ref().to_string_lossy())))
        }
    };
    let mut buff = String::new();
    file.read_to_string(&mut buff)?;

    Ok(buff)
}

pub fn write_file<T: AsRef<Path>>(path: T, data: &str) -> Result<(), Error> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)?;
    f.write_all(data.as_bytes())?;
    f.flush()?;

    Ok(())
}

pub fn mdpath2html(path: &str, headerids: bool) -> Result<String, Error> {
    let mut comrak_options = Options::default();
    comrak_options.render.unsafe_ = true;
    comrak_options.extension.table = true;
    if headerids {
        comrak_options.extension.header_ids = Some("".to_string());
    }
    let markdown = match read_file(path) {
        Ok(c) => c,
        Err(e) => return Err(Error::new(ErrorKind::Other, format!("Failed to render markdown file {path}: {e}")))
    };
    let content = markdown_to_html(&markdown, &comrak_options);

    Ok(content)
}

pub fn loadconfig() -> Result<SiteConfig, Error> {
    let mut siteconfig: SiteConfig = serde_json::from_str(&read_file("ctclsite-config/config.json").unwrap()).unwrap();

    mkdir("static/")?;

    siteconfig.fonts = match loadfonts(&siteconfig) {
        Ok(t) => t,
        Err(e) => return Err(e)
    }; 

    // Themes must be loaded before pages are loaded
    siteconfig.themes = match loadthemes(&siteconfig) {
        Ok(t) => t,
        Err(e) => return Err(e)
    };

    mkfavicons(&siteconfig.themes)?;
    match collectstatic(&siteconfig) {
        Ok(_) => (),
        Err(e) => return Err(Error::new(ErrorKind::Other, format!("collectstatic: {e}")))
    };

    // Catch-22: Pages must be loaded to load pages in order to fill in linklist entries with information of a page
    siteconfig.pages = loadpages(&siteconfig)?;
    
    if siteconfig.pages.is_empty() {
        error!("No pages found");
        return Err(Error::new(ErrorKind::NotFound, "No pages found"));
    }
    if siteconfig.themes.is_empty() {
        error!("No themes found");
        return Err(Error::new(ErrorKind::NotFound, "No themes found"));
    }

    Ok(siteconfig)
}