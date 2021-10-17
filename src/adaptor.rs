use crate::utils;

use pulldown_cmark as md;
use std::collections::HashSet;

pub trait AdaptorExt<'a>
where
    Self: Sized + Iterator<Item = md::Event<'a>>,
{
    fn hyperlink_headings(self) -> HyperlinkHeadings<'a, Self> {
        HyperlinkHeadings {
            head: None,
            iter: self,
            generated_ids: HashSet::new(),
        }
    }
}

impl<'a, I> AdaptorExt<'a> for I where I: Iterator<Item = md::Event<'a>> {}

pub struct HyperlinkHeadings<'a, I>
where
    I: Iterator<Item = md::Event<'a>>,
{
    head: Option<md::Event<'a>>,
    iter: I,
    generated_ids: HashSet<String>,
}

impl<'a, I> Iterator for HyperlinkHeadings<'a, I>
where
    I: Iterator<Item = md::Event<'a>>,
{
    type Item = md::Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.head.take() {
            Some(item) => return Some(item),
            None => {}
        }

        match self.iter.next() {
            Some(md::Event::Start(md::Tag::Heading(level))) => match self.iter.next() {
                Some(md::Event::Text(text)) => {
                    let mut id = utils::generate_heading_id(&text);
                    if self.generated_ids.contains(&id) {
                        let original_id = id.clone();
                        let mut i = 1;
                        while self.generated_ids.contains(&id) {
                            i += 1;
                            id = format!("{}{}", original_id, i);
                        }
                    }

                    let heading = Some(md::Event::Html(
                        format!("<h{} id=\"{}\">", level, id).into(),
                    ));
                    self.head = Some(md::Event::Text(text));
                    self.generated_ids.insert(id);
                    heading
                }
                Some(item) => {
                    self.head = Some(item);
                    Some(md::Event::Start(md::Tag::Heading(level)))
                }
                None => None,
            },
            item => item,
        }
    }
}
