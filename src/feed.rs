use crate::{Post, FEED_CONTENT_TYPE, FEED_REL, FEED_TYPE};

use atom_syndication as atom;
use pulldown_cmark as md;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::PathBuf;

enum State {
    WaitFeed,
    WaitInfo,
    WaitTitle,
    WaitGenerator,
}

pub struct Meta {
    pub path: PathBuf,
    title: String,
    link: String,
    lang: Option<String>,
    generator: Option<String>,
    generator_uri: Option<String>,
}

macro_rules! match_or_continue {
    ( $event_ty:ident ( $event:ident ) ) => {
        match $event {
            Event::$event_ty(e) => e,
            Event::Eof => break,
            _ => continue,
        }
    };
    ( $event_ty:ident ( $event:ident ) if $guard:expr ) => {
        match $event {
            Event::$event_ty($event) if $guard => $event,
            Event::Eof => break,
            _ => continue,
        }
    };
}

pub fn load_atom_feed(path: &PathBuf) -> quick_xml::Result<Meta> {
    let mut reader = Reader::from_file(path)?;
    let mut buffer = Vec::new();
    let mut state = State::WaitFeed;

    let mut title = None;
    let mut link = None;
    let mut lang = None;
    let mut generator = None;
    let mut generator_uri = None;

    loop {
        buffer.clear();
        let event = reader.read_event(&mut buffer)?;
        state = match state {
            State::WaitFeed => {
                let e = match_or_continue!(Start(event) if event.name() == b"feed");
                for attr in e.attributes() {
                    let attr = attr?;
                    if attr.key == b"xml:lang" {
                        lang = Some(attr.unescape_and_decode_value(&reader)?);
                    }
                }
                State::WaitInfo
            }
            State::WaitInfo => match event {
                Event::Start(e) if e.name() == b"title" => State::WaitTitle,
                Event::Start(e) if e.name() == b"generator" => {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key == b"uri" {
                            generator_uri = Some(attr.unescape_and_decode_value(&reader)?);
                        }
                    }
                    State::WaitGenerator
                }
                Event::Start(e) | Event::Empty(e) if e.name() == b"link" => {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key == b"href" {
                            link = Some(attr.unescape_and_decode_value(&reader)?);
                        }
                    }
                    continue;
                }
                Event::Eof => break,
                _ => continue,
            },
            State::WaitTitle => {
                title = Some(match_or_continue!(Text(event)).unescape_and_decode(&reader)?);
                State::WaitInfo
            }
            State::WaitGenerator => {
                generator = Some(match_or_continue!(Text(event)).unescape_and_decode(&reader)?);
                State::WaitInfo
            }
        };
    }

    let title = match title {
        Some(t) => t,
        None => {
            eprintln!(
                "note: atom feed lacks title tag, treating as invalid: {:?}",
                path
            );
            return Err(quick_xml::Error::TextNotFound);
        }
    };

    let link = match link {
        Some(t) => t,
        None => {
            eprintln!(
                "note: atom feed lacks link tag, treating as invalid: {:?}",
                path
            );
            return Err(quick_xml::Error::TextNotFound);
        }
    };

    Ok(Meta {
        path: path.clone(),
        title,
        link,
        lang,
        generator,
        generator_uri,
    })
}

pub fn fill_atom_feed(feed: Meta, md_files: &Vec<Post>) -> String {
    let parent = feed.path.parent().unwrap();

    let mut entries = Vec::new();
    let mut last_updated = None;

    for md in md_files {
        if md.path.starts_with(parent) {
            if let Some(updated) = last_updated {
                last_updated = Some(md.updated.max(updated));
            } else {
                last_updated = Some(md.updated);
            }

            entries.push(atom::Entry {
                title: md.title.clone().into(),
                id: {
                    let mut s = feed.link.clone();
                    s.push_str(&md.uri);
                    s
                },
                updated: md.updated.and_hms(0, 0, 0).into(),
                published: Some(md.date.and_hms(0, 0, 0).into()),
                categories: vec![atom::Category {
                    term: md.category.clone(),
                    ..atom::Category::default()
                }],
                content: Some(atom::Content {
                    value: {
                        let mut html = String::new();
                        md::html::push_html(&mut html, md::Parser::new(&md.markdown));
                        let mut escaped = String::new();
                        md::escape::escape_html(&mut escaped, &html).unwrap();
                        Some(escaped)
                    },
                    content_type: Some(FEED_CONTENT_TYPE.to_string()),
                    ..atom::Content::default()
                }),
                ..atom::Entry::default()
            });
        }
    }

    let mut self_link = feed.link.trim_end_matches('/').to_owned();
    self_link.push('/');
    self_link.push_str(&feed.path.file_name().unwrap().to_str().unwrap());

    if let Some(lang) = feed.lang.as_ref() {
        eprintln!(
            "note: feed lang '{}' is currently ignored: see gh/atom/issues/54",
            lang
        );
    }

    return atom::Feed {
        title: feed.title.clone().into(),
        id: feed.link.clone(),
        updated: last_updated
            .map(|d| d.and_hms(0, 0, 0).into())
            .unwrap_or_else(|| chrono::offset::Local::now().into()),
        entries,
        generator: feed.generator.clone().map(|value| atom::Generator {
            value,
            uri: feed.generator_uri.clone(),
            ..atom::Generator::default()
        }),
        links: vec![
            atom::Link {
                href: feed.link,
                ..atom::Link::default()
            },
            atom::Link {
                href: self_link,
                rel: FEED_REL.into(),
                mime_type: Some(FEED_TYPE.to_owned()),
                ..atom::Link::default()
            },
        ],
        ..atom::Feed::default()
    }
    .to_string();
}
