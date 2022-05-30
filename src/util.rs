use std::str::FromStr;

use image;
use mime::Mime;

// XXX: Move into Piston?
pub trait AsImageFormat {
    fn parse_image_format(&self) -> Option<(Mime, image::ImageFormat)>;
}

impl AsImageFormat for Mime {
    fn parse_image_format(&self) -> Option<(Mime, image::ImageFormat)> {
        if *self == mime::IMAGE_PNG {
            Some((self.clone(), image::ImageFormat::Png))
        } else if *self == mime::IMAGE_JPEG {
            Some((self.clone(), image::ImageFormat::Jpeg))
        } else if *self == mime::IMAGE_GIF {
            Some((self.clone(), image::ImageFormat::Gif))
        } else if self.subtype() == "x-icon" || self.subtype() == "vnd.microsoft.icon" {
            return Some((
                Mime::from_str("image/x-icon").unwrap(),
                image::ImageFormat::Ico,
            ));
        } else {
            return None;
        }
    }
}
