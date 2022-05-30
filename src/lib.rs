// DOCS

#[macro_use]
extern crate error_chain;
extern crate html5ever;
extern crate html5ever_atoms;
extern crate image;
extern crate mime;
extern crate reqwest;
extern crate url;

pub mod errors;
mod strategies;
mod util;

use errors::*;
use reqwest::IntoUrl;
use scraper::Html;
use std::str::FromStr;
use strategies::Strategy;
use util::AsImageFormat;

pub struct IconScraper {
    document_url: url::Url,
    dom: Option<Html>,
}

impl IconScraper {
    pub async fn from_http<I: IntoUrl>(url: I) -> Self {
        let url = url.into_url().unwrap();
        let dom = reqwest::get(url.clone())
            .await
            .unwrap()
            .text()
            .await
            .and_then(|text| Ok(Html::parse_document(&text)))
            .ok();

        IconScraper {
            document_url: url,
            dom: dom,
        }
    }

    /// Search the document for icon metadata, also brute-force some favicon paths.
    ///
    /// **Note:** This operation is fairly costly, it is recommended to cache the results!
    ///
    /// # Panics
    ///
    /// If the document is not fetched yet.
    pub async fn fetch_icons(&mut self) -> IconCollection {
        let futures = strategies::LinkRelStrategy
            .get_guesses(self)
            .await
            .into_iter()
            .chain(
                strategies::DefaultFaviconPathStrategy
                    .get_guesses(self)
                    .await
                    .into_iter(),
            )
            .map(|mut icon| async {
                if icon.fetch_dimensions().await.is_ok() {
                    Some(icon)
                } else {
                    None
                }
            });
        let icons = futures::future::join_all(futures)
            .await
            .into_iter()
            .filter_map(|x| x)
            .collect::<Vec<_>>();

        IconCollection::from_raw(icons)
    }
}

pub struct IconCollection {
    icons: Vec<Icon>,
}

impl IconCollection {
    fn from_raw(mut icons: Vec<Icon>) -> Self {
        icons.sort_by(|a, b| {
            (a.width.unwrap() * a.height.unwrap()).cmp(&(b.width.unwrap() * b.height.unwrap()))
        });
        IconCollection { icons: icons }
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

#[derive(Clone)]
pub struct Icon {
    pub url: url::Url,
    pub raw: Option<Vec<u8>>,
    pub mime_type: Option<mime::Mime>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Icon {
    pub fn from_url(url: url::Url) -> Self {
        Icon {
            url: url,
            raw: None,
            mime_type: None,
            width: None,
            height: None,
        }
    }

    pub async fn fetch(&mut self) -> Result<()> {
        if self.raw.is_some() {
            return Ok(());
        };

        let response = reqwest::get(self.url.clone()).await?;
        if !response.status().is_success() {
            return Err(ErrorKind::BadStatusCode(response).into());
        }

        let mime_type: mime::Mime = match response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .cloned()
        {
            Some(x) => match x.to_str() {
                Ok(s) => match mime::Mime::from_str(s) {
                    Ok(m) => m,
                    Err(_) => return Err(ErrorKind::BadContentType(response).into()),
                },
                Err(_) => return Err(ErrorKind::NoContentType(response).into()),
            },
            None => return Err(ErrorKind::NoContentType(response).into()),
        };
        let (better_mime_type, image_format) = match mime_type.parse_image_format() {
            Some(x) => x,
            None => return Err(ErrorKind::BadContentType(response).into()),
        };

        let bytes: Vec<u8> = response.bytes().await?.to_vec();
        let image = image::load_from_memory_with_format(&bytes, image_format)?;

        self.width = Some(image.width());
        self.height = Some(image.height());
        self.raw = Some(bytes);
        self.mime_type = Some(better_mime_type);
        Ok(())
    }

    pub async fn fetch_dimensions(&mut self) -> Result<()> {
        match (self.width, self.height) {
            (Some(_), Some(_)) => Ok(()),
            _ => self.fetch().await,
        }
    }
}
