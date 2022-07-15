use super::{Icon, IconScraper};
use async_trait::async_trait;
use scraper::Selector;
use std::str::FromStr;

#[async_trait]
pub trait Strategy {
    fn get_guesses(self, parser: &mut IconScraper) -> Vec<Icon>;
}

pub struct DefaultFaviconPathStrategy;

#[async_trait]
impl Strategy for DefaultFaviconPathStrategy {
    fn get_guesses(self, parser: &mut IconScraper) -> Vec<Icon> {
        let icon = Icon::from_url(parser.document_url.join("/favicon.ico").unwrap());
        vec![icon]
    }
}

pub struct LinkRelStrategy;
impl Strategy for LinkRelStrategy {
    fn get_guesses(self, parser: &mut IconScraper) -> Vec<Icon> {
        let mut rv = vec![];
        let dom = match parser.dom {
            Some(ref x) => x,
            None => return rv,
        };

        for data in dom.select(&Selector::try_from("link[rel*=icon]").unwrap()) {
            let href = match data.value().attr("href") {
                Some(x) => x,
                None => continue,
            };

            let icon_url = match parser.document_url.join(href) {
                Ok(x) => x,
                Err(_) => continue,
            };

            let mut sizes = data
                .value()
                .attr("sizes")
                .unwrap_or("")
                .split('x')
                .filter_map(|d| u32::from_str(d).ok());

            let (x, y) = match (sizes.next(), sizes.next()) {
                (Some(x), Some(y)) => (Some(x), Some(y)),
                _ => (None, None),
            };

            rv.push({
                let mut icon = Icon::from_url(icon_url);
                icon.width = x;
                icon.height = y;
                icon
            });
        }

        rv
    }
}

#[cfg(test)]
mod tests {
    use super::super::IconScraper;
    use super::*;

    use scraper::Html;
    use url;

    #[test]
    fn test_apple_touch_icon_without_size_attr() {
        // laverna.cc does this.
        let mut scraper = IconScraper {
            document_url: url::Url::parse("http://example.com/").unwrap(),
            dom: Some(Html::parse_document(
                "<!DOCTYPE html>
            <html>
                <head>
                    <link rel=apple-touch-icon href=apple-touch-icon.png>
                </head>
                <body></body>
            </html>
            ",
            )),
        };

        let mut icons = LinkRelStrategy.get_guesses(&mut scraper);
        assert_eq!(icons.len(), 1);
        assert_eq!(
            icons.pop().unwrap().url,
            url::Url::parse("http://example.com/apple-touch-icon.png").unwrap()
        );
    }

    #[test]
    fn test_sharesome() {
        assert_eq!(
            tokio_test::block_on(IconScraper::fetch_icons("https://sharesome.5apps.com/"))
                .largest()
                .unwrap()
                .url,
            url::Url::parse("https://sharesome.5apps.com/application_icon_x512.png").unwrap()
        );
    }
}
