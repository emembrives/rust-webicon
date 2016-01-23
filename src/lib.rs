// DOCS

#[macro_use] extern crate quick_error;
#[macro_use] extern crate string_cache;
extern crate mime;
extern crate hyper;
extern crate url;
extern crate kuchiki;
extern crate image;

mod strategies;
mod util;

use image::GenericImage;
use strategies::Strategy;
use std::io::Read;
use util::AsImageFormat;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Hyper(error: hyper::Error) { from() }
        Io(error: std::io::Error) { from() }
        Image(error: image::ImageError) { from() }
        Other(msg: String) {
            description(msg)
        }
    }
}


pub struct IconScraper {
    document_url: url::Url,
    dom: Option<kuchiki::NodeRef>,
}

impl IconScraper {
    pub fn from_url(url: url::Url) -> Self {
        let client = hyper::client::Client::new();
        let mut response = client.get(url.clone()).send().ok();
        IconScraper::from_url_and_stream(url, response.as_mut())
    }

    pub fn from_url_and_stream<R: Read>(url: url::Url, stream: Option<&mut R>) -> Self {
        IconScraper {
            document_url: url,
            dom: stream
                .and_then(|stream| kuchiki::Html::from_stream(stream).ok())
                .map(|x| x.parse())
        }
    }

    pub fn from_url_and_dom(url: url::Url, dom: kuchiki::NodeRef) -> Self {
        IconScraper {
            document_url: url,
            dom: Some(dom)
        }
    }

    /// Search the document for icon metadata, also brute-force some favicon paths.
    ///
    /// **Note:** This operation is fairly costly, it is recommended to cache the results!
    ///
    /// # Panics
    ///
    /// If the document is not fetched yet.
    pub fn fetch_icons(&mut self) -> IconCollection {
        let icons = strategies::LinkRelStrategy.get_guesses(self)
            .into_iter()
            .chain(strategies::DefaultFaviconPathStrategy.get_guesses(self).into_iter())
            .filter_map(|mut icon| if icon.fetch_dimensions().is_ok() { Some(icon) } else { None })
            .collect::<Vec<_>>();

        IconCollection::from_raw(icons)
    }
}

pub struct IconCollection {
    icons: Vec<Icon>
}

impl IconCollection {
    fn from_raw(mut icons: Vec<Icon>) -> Self {
        icons.sort_by(|a, b| {
            (a.width.unwrap() * a.height.unwrap())
                .cmp(&(b.width.unwrap() * b.height.unwrap()))
        });
        IconCollection {
            icons: icons
        }
    }

    /// Return an icon that is at least of the given dimensions
    ///
    /// If there's only one icon available, it will return that icon. If there's no icon available,
    /// None is returned.
    pub fn at_least(mut self, width: u32, height: u32) -> Option<Icon> {
        let largest = self.icons.pop();
        self.icons
            .into_iter()
            .skip_while(|icon| icon.width.unwrap() < width || icon.height.unwrap() < height)
            .next()
            .or(largest)
    }

    /// Return the largest icon
    pub fn largest(mut self) -> Option<Icon> {
        self.icons.pop()
    }

    /// [unstable] Give up ownership of the inner datastructure: A vector of icons, sorted
    /// ascendingly by size
    pub fn into_raw_parts(self) -> Vec<Icon> {
        self.icons
    }
}

pub struct Icon {
    pub url: url::Url,
    pub raw: Option<Vec<u8>>,
    pub mime_type: Option<mime::Mime>,
    pub width: Option<u32>,
    pub height: Option<u32>
}

impl Icon {
    pub fn from_url(url: url::Url) -> Self {
        Icon {
            url: url,
            raw: None,
            mime_type: None,
            width: None,
            height: None
        }
    }

    pub fn fetch(&mut self) -> Result<(), Error> {
        if self.raw.is_some() {
            return Ok(());
        };

        let client = hyper::client::Client::new();
        let mut response = try!(client.get(self.url.clone()).send());

        let mut bytes: Vec<u8> = vec![];
        try!(response.read_to_end(&mut bytes));
        if !response.status.is_success() {
            return Err(Error::Other(format!("Bad status code: {:?}", response.status)));
        }

        let mime_type: mime::Mime = match response.headers.get::<hyper::header::ContentType>() {
            Some(x) => x.clone().0,
            None => return Err(Error::Other("No Content-Type found.".to_owned()))
        };
        let image_format = match mime_type.as_image_format() {
            Some(x) => x,
            None => return Err(Error::Other(format!("Invalid image type: {:?}", mime_type)))
        };
        let image = try!(image::load_from_memory_with_format(&bytes, image_format));


        self.width = Some(image.width());
        self.height = Some(image.height());
        self.raw = Some(bytes);
        self.mime_type = Some(mime_type);
        Ok(())
    }

    pub fn fetch_dimensions(&mut self) -> Result<(), Error> {
        match (self.width, self.height) {
            (Some(_), Some(_)) => Ok(()),
            _ => self.fetch()
        }
    }
}
