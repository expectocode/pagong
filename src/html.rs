// Copyright 2015 Google Inc. All rights reserved.
// Modifications Copyright 2020 Tortuga Authors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! HTML renderer that takes an iterator of events as input.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Arguments, Write as FmtWrite};
use std::io::{self, ErrorKind, Write};

use crate::escape::{escape_href, escape_html};

use pulldown_cmark::Event::*;
use pulldown_cmark::{Alignment, CodeBlockKind, CowStr, Event, LinkType, Tag};

enum TableState {
    Head,
    Body,
}

/// This wrapper exists because we can't have both a blanket implementation
/// for all types implementing `Write` and types of the for `&mut W` where
/// `W: StrWrite`. Since we need the latter a lot, we choose to wrap
/// `Write` types.
struct WriteWrapper<W>(W);

/// Trait that allows writing string slices. This is basically an extension
/// of `std::io::Write` in order to include `String`.
pub(crate) trait StrWrite {
    fn write_str(&mut self, s: &str) -> io::Result<()>;

    fn write_fmt(&mut self, args: Arguments) -> io::Result<()>;
}

impl<W> StrWrite for WriteWrapper<W>
where
    W: Write,
{
    #[inline]
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.0.write_all(s.as_bytes())
    }

    #[inline]
    fn write_fmt(&mut self, args: Arguments) -> io::Result<()> {
        self.0.write_fmt(args)
    }
}

impl<'w> StrWrite for String {
    #[inline]
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, args: Arguments) -> io::Result<()> {
        // FIXME: translate fmt error to io error?
        FmtWrite::write_fmt(self, args).map_err(|_| ErrorKind::Other.into())
    }
}

impl<W> StrWrite for &'_ mut W
where
    W: StrWrite,
{
    #[inline]
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        (**self).write_str(s)
    }

    #[inline]
    fn write_fmt(&mut self, args: Arguments) -> io::Result<()> {
        (**self).write_fmt(args)
    }
}

struct ImageParagraphFilter<I: Iterator> {
    iter: I,
    queue: VecDeque<I::Item>,
}

impl<'a, I: Iterator> ImageParagraphFilter<I> {
    fn new(iter: I) -> Self {
        Self {
            iter,
            queue: VecDeque::new(),
        }
    }

    /// Ensure there are `n` items in the queue, unless the iterator has been
    /// exhausted.
    fn lookahead_n(&mut self, n: usize) {
        while self.queue.len() < n {
            if let Some(next) = self.iter.next() {
                self.queue.push_back(next);
            } else {
                break;
            }
        }
    }
}

impl<'a, I> Iterator for ImageParagraphFilter<I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = (Event<'a>, bool); // Hack for standalone images bool

    fn next(&mut self) -> Option<Self::Item> {
        // We want to check 5 ahead because that's open-paragraph, image, text,
        // close-image, close-paragraph.
        self.lookahead_n(5);
        let first = self.queue.pop_front();

        let buffer = [
            first.as_ref(),
            self.queue.get(0),
            self.queue.get(1),
            self.queue.get(2),
            self.queue.get(3),
        ];

        use Tag::{Image, Paragraph};

        #[rustfmt::skip]
        let res = match buffer {
            [
                Some(Start(Paragraph)),
                Some(Start(Image(..))),
                Some(Text(_)),
                Some(End(Image(..))),
                Some(End(Paragraph))
            ] => {
                let im_start = self.queue.pop_front().unwrap();
                let im_text = self.queue.pop_front().unwrap();
                let im_end = self.queue.pop_front().unwrap();
                self.queue.pop_front(); // end paragraph
                self.queue.push_front(im_end);
                self.queue.push_front(im_text);

                Some((im_start, true))
            }
            _ => first.map(|a| (a, false)),
        };

        res
    }
}

struct HtmlWriter<'a, I, W> {
    /// Iterator supplying events.
    iter: I,

    /// Writer to write to.
    writer: W,

    /// Have we written out the post's title yet?
    title_written: bool,

    /// Are we expecting to write a heading's text next?
    expecting_heading_text: bool,

    /// Heading identifiers generated so far.
    heading_identifiers: HashSet<String>,

    /// Are we inside a footnote's definition?
    inside_footnote_def: bool,

    /// Whether or not the last write wrote a newline.
    end_newline: bool,

    table_state: TableState,
    table_alignments: Vec<Alignment>,
    table_cell_index: usize,
    numbers: HashMap<CowStr<'a>, usize>,
}

impl<'a, I, W> HtmlWriter<'a, ImageParagraphFilter<I>, W>
where
    I: Iterator<Item = Event<'a>>,
    W: StrWrite,
{
    fn new(iter: I, writer: W) -> Self {
        Self {
            iter: ImageParagraphFilter::new(iter),
            writer,
            title_written: false,
            expecting_heading_text: false,
            heading_identifiers: HashSet::new(),
            inside_footnote_def: false,
            end_newline: true,
            table_state: TableState::Head,
            table_alignments: vec![],
            table_cell_index: 0,
            numbers: HashMap::new(),
        }
    }

    /// Generate a new unique identifier for a heading.
    fn generate_heading_id(&mut self, heading: &str) -> String {
        const SEP_CHAR: char = '_';

        let mut identifier = String::with_capacity(heading.len());
        let mut ignored_last = false;
        heading.chars().for_each(|c| {
            if c.is_alphanumeric() {
                ignored_last = false;
                c.to_lowercase().for_each(|lc| identifier.push(lc));
            } else if !ignored_last {
                ignored_last = true;
                identifier.push(SEP_CHAR);
            }
        });

        if self.heading_identifiers.insert(identifier.clone()) {
            return identifier;
        }

        for n in 2.. {
            let new_identifier = format!("{}{}{}", identifier, SEP_CHAR, n);
            if self.heading_identifiers.insert(new_identifier.clone()) {
                return new_identifier;
            }
        }

        panic!(format!(
            "user somehow wrote {} identically-named headings",
            usize::MAX - 2
        ));
    }

    /// Writes a new line.
    fn write_newline(&mut self) -> io::Result<()> {
        self.end_newline = true;
        self.writer.write_str("\n")
    }

    /// Writes a buffer, and tracks whether or not a newline was written.
    #[inline]
    fn write(&mut self, s: &str) -> io::Result<()> {
        self.writer.write_str(s)?;

        if !s.is_empty() {
            self.end_newline = s.ends_with('\n');
        }
        Ok(())
    }

    pub fn run(mut self) -> io::Result<()> {
        while let Some((event, is_standalone)) = self.iter.next() {
            match event {
                Start(tag) => {
                    self.start_tag(tag, is_standalone)?;
                }
                End(tag) => {
                    self.end_tag(tag)?;
                }
                Text(text) => {
                    if self.expecting_heading_text {
                        self.expecting_heading_text = false;
                        let identifier = self.generate_heading_id(&text);
                        write!(&mut self.writer, "{}\">", identifier)?;
                        write!(
                            &mut self.writer,
                            "<a class=\"anchor\" href=\"#{}\">¶</a>",
                            identifier
                        )?;
                    }
                    escape_html(&mut self.writer, &text)?;
                    self.end_newline = text.ends_with('\n');
                }
                Code(text) => {
                    self.write("<code>")?;
                    escape_html(&mut self.writer, &text)?;
                    self.write("</code>")?;
                }
                Html(html) => {
                    self.write(&html)?;
                }
                SoftBreak => {
                    self.write_newline()?;
                }
                HardBreak => {
                    self.write("<br />\n")?;
                }
                Rule => {
                    if self.end_newline {
                        self.write("<hr />\n")?;
                    } else {
                        self.write("\n<hr />\n")?;
                    }
                }
                FootnoteReference(name) => {
                    let len = self.numbers.len() + 1;
                    let number = *self.numbers.entry(name.clone()).or_insert(len);
                    write!(
                        &mut self.writer,
                        "<sup class=\"footnote-reference\" id=\"r.{}\"><a href=\"#f.",
                        name
                    )?;
                    escape_html(&mut self.writer, &name)?;
                    self.write("\">")?;
                    write!(&mut self.writer, "{}", number)?;
                    self.write("</a></sup>")?;
                }
                TaskListMarker(true) => {
                    self.write("<input disabled=\"\" type=\"checkbox\" checked=\"\"/>\n")?;
                }
                TaskListMarker(false) => {
                    self.write("<input disabled=\"\" type=\"checkbox\"/>\n")?;
                }
            }
        }
        Ok(())
    }

    /// Writes the start of an HTML tag.
    fn start_tag(&mut self, tag: Tag<'a>, is_standalone: bool) -> io::Result<()> {
        match tag {
            Tag::Paragraph => {
                if self.inside_footnote_def {
                    Ok(())
                } else if self.end_newline {
                    self.write("<p>")
                } else {
                    self.write("\n<p>")
                }
            }
            Tag::Heading(level) => {
                if self.end_newline {
                    self.end_newline = false;
                } else {
                    self.writer.write_str("\n")?;
                }
                write!(&mut self.writer, "<h{}", level)?;
                if !self.title_written {
                    self.title_written = true;
                    self.writer.write_str(" class=\"title\"")?;
                }
                self.expecting_heading_text = true;
                write!(&mut self.writer, " id=\"")
            }
            Tag::Table(alignments) => {
                self.table_alignments = alignments;
                self.write("<table>")
            }
            Tag::TableHead => {
                self.table_state = TableState::Head;
                self.table_cell_index = 0;
                self.write("<thead><tr>")
            }
            Tag::TableRow => {
                self.table_cell_index = 0;
                self.write("<tr>")
            }
            Tag::TableCell => {
                match self.table_state {
                    TableState::Head => {
                        self.write("<th")?;
                    }
                    TableState::Body => {
                        self.write("<td")?;
                    }
                }
                match self.table_alignments.get(self.table_cell_index) {
                    Some(&Alignment::Left) => self.write(" align=\"left\">"),
                    Some(&Alignment::Center) => self.write(" align=\"center\">"),
                    Some(&Alignment::Right) => self.write(" align=\"right\">"),
                    _ => self.write(">"),
                }
            }
            Tag::BlockQuote => {
                if self.end_newline {
                    self.write("<blockquote>\n")
                } else {
                    self.write("\n<blockquote>\n")
                }
            }
            Tag::CodeBlock(info) => {
                if !self.end_newline {
                    self.write_newline()?;
                }
                match info {
                    CodeBlockKind::Fenced(info) => {
                        let lang = info.split(' ').next().unwrap();
                        if lang.is_empty() {
                            self.write("<pre><code>")
                        } else {
                            self.write("<pre><code class=\"language-")?;
                            escape_html(&mut self.writer, lang)?;
                            self.write("\">")
                        }
                    }
                    CodeBlockKind::Indented => self.write("<pre><code>"),
                }
            }
            Tag::List(Some(1)) => {
                if self.end_newline {
                    self.write("<ol>\n")
                } else {
                    self.write("\n<ol>\n")
                }
            }
            Tag::List(Some(start)) => {
                if self.end_newline {
                    self.write("<ol start=\"")?;
                } else {
                    self.write("\n<ol start=\"")?;
                }
                write!(&mut self.writer, "{}", start)?;
                self.write("\">\n")
            }
            Tag::List(None) => {
                if self.end_newline {
                    self.write("<ul>\n")
                } else {
                    self.write("\n<ul>\n")
                }
            }
            Tag::Item => {
                if self.end_newline {
                    self.write("<li>")
                } else {
                    self.write("\n<li>")
                }
            }
            Tag::Emphasis => self.write("<em>"),
            Tag::Strong => self.write("<strong>"),
            Tag::Strikethrough => self.write("<del>"),
            Tag::Link(LinkType::Email, dest, title) => {
                self.write("<a href=\"mailto:")?;
                escape_href(&mut self.writer, &dest)?;
                if !title.is_empty() {
                    self.write("\" title=\"")?;
                    escape_html(&mut self.writer, &title)?;
                }
                self.write("\">")
            }
            Tag::Link(_link_type, dest, title) => {
                self.write("<a href=\"")?;
                escape_href(&mut self.writer, &dest)?;
                if !title.is_empty() {
                    self.write("\" title=\"")?;
                    escape_html(&mut self.writer, &title)?;
                }
                self.write("\">")
            }
            Tag::Image(_link_type, dest, title) => {
                if is_standalone {
                    self.write("<div class=\"image-container\">\n")?;
                }
                self.write("<img src=\"")?;
                escape_href(&mut self.writer, &dest)?;
                self.write("\" alt=\"")?;
                self.raw_text()?;
                if !title.is_empty() {
                    self.write("\" title=\"")?;
                    escape_html(&mut self.writer, &title)?;
                }
                self.write("\" />")?;

                if is_standalone {
                    self.write("\n<div class=\"image-caption\">")?;
                    escape_html(&mut self.writer, &title)?;
                    self.write("</div>\n")?;
                    self.write("</div>\n")?;
                    self.write("<p>\n")?;
                }

                Ok(())
            }
            Tag::FootnoteDefinition(name) => {
                self.inside_footnote_def = true;
                if self.end_newline {
                    self.write("<p class=\"footnote\" id=\"f.")?;
                } else {
                    self.write("\n<p class=\"footnote\" id=\"f.")?;
                }
                escape_html(&mut self.writer, &*name)?;
                self.write("\"><sup>")?;
                let len = self.numbers.len() + 1;
                let number = *self.numbers.entry(name).or_insert(len);
                write!(&mut self.writer, "{}", number)?;
                self.write("</sup> ")
            }
        }
    }

    fn end_tag(&mut self, tag: Tag) -> io::Result<()> {
        match tag {
            Tag::Paragraph => {
                if !self.inside_footnote_def {
                    self.write("</p>\n")?;
                }
            }
            Tag::Heading(level) => {
                self.write("</h")?;
                write!(&mut self.writer, "{}", level)?;
                self.write(">\n")?;
            }
            Tag::Table(_) => {
                self.write("</tbody></table>\n")?;
            }
            Tag::TableHead => {
                self.write("</tr></thead><tbody>\n")?;
                self.table_state = TableState::Body;
            }
            Tag::TableRow => {
                self.write("</tr>\n")?;
            }
            Tag::TableCell => {
                match self.table_state {
                    TableState::Head => {
                        self.write("</th>")?;
                    }
                    TableState::Body => {
                        self.write("</td>")?;
                    }
                }
                self.table_cell_index += 1;
            }
            Tag::BlockQuote => {
                self.write("</blockquote>\n")?;
            }
            Tag::CodeBlock(_) => {
                self.write("</code></pre>\n")?;
            }
            Tag::List(Some(_)) => {
                self.write("</ol>\n")?;
            }
            Tag::List(None) => {
                self.write("</ul>\n")?;
            }
            Tag::Item => {
                self.write("</li>\n")?;
            }
            Tag::Emphasis => {
                self.write("</em>")?;
            }
            Tag::Strong => {
                self.write("</strong>")?;
            }
            Tag::Strikethrough => {
                self.write("</del>")?;
            }
            Tag::Link(_, _, _) => {
                self.write("</a>")?;
            }
            Tag::Image(_, _, _) => (), // shouldn't happen, handled in start
            Tag::FootnoteDefinition(name) => {
                self.inside_footnote_def = false;
                write!(&mut self.writer, " <a href=\"#r.{}\">↩</a></p>\n", name)?;
            }
        }
        Ok(())
    }

    // run raw text, consuming end tag
    fn raw_text(&mut self) -> io::Result<()> {
        let mut nest = 0;
        while let Some((event, _)) = self.iter.next() {
            match event {
                Start(_) => nest += 1,
                End(_) => {
                    if nest == 0 {
                        break;
                    }
                    nest -= 1;
                }
                Html(text) | Code(text) | Text(text) => {
                    escape_html(&mut self.writer, &text)?;
                    self.end_newline = text.ends_with('\n');
                }
                SoftBreak | HardBreak | Rule => {
                    self.write(" ")?;
                }
                FootnoteReference(name) => {
                    let len = self.numbers.len() + 1;
                    let number = *self.numbers.entry(name).or_insert(len);
                    write!(&mut self.writer, "[{}]", number)?;
                }
                TaskListMarker(true) => self.write("[x]")?,
                TaskListMarker(false) => self.write("[ ]")?,
            }
        }
        Ok(())
    }
}

/// Iterate over an `Iterator` of `Event`s, generate HTML for each `Event`, and
/// push it to a `String`.
///
/// # Examples
///
/// ```
/// use pulldown_cmark::{html, Parser};
///
/// let markdown_str = r#"
/// hello
/// =====
///
/// * alpha
/// * beta
/// "#;
/// let parser = Parser::new(markdown_str);
///
/// let mut html_buf = String::new();
/// html::push_html(&mut html_buf, parser);
///
/// assert_eq!(html_buf, r#"<h1>hello</h1>
/// <ul>
/// <li>alpha</li>
/// <li>beta</li>
/// </ul>
/// "#);
/// ```
pub fn push_html<'a, I>(s: &mut String, iter: I)
where
    I: Iterator<Item = Event<'a>>,
{
    HtmlWriter::new(iter, s).run().unwrap();
}

#[cfg(test)]
mod test {
    use super::*;
    use pulldown_cmark::{Options, Parser};

    #[test]
    fn standalone_images_are_not_paragraphs() {
        let input = "Some text above

![image alt](image_url \"Image Title\")

Some text below.";

        let parser = Parser::new(input);

        let filtered_events: Vec<_> = ImageParagraphFilter::new(parser).collect();

        let image_paragraph = &filtered_events[3..(filtered_events.len() - 3)];

        assert!(matches!(
            image_paragraph,
            [(Start(Tag::Image(..)), true), (Text(_), false), (End(Tag::Image(..)), false)]
        ));
    }

    #[test]
    fn check_footnotes() {
        let input = "Some text above

Introducing a footnote[^foot].

Some text below.

[^foot]: Footnote explanation.
";

        let parser = Parser::new_ext(input, Options::all());

        let html = {
            let mut buffer = String::new();
            push_html(&mut buffer, parser);
            buffer
        };

        assert!(html.contains("id=\"r.foot"), "missing return id");
        assert!(html.contains("id=\"f.foot"), "missing footnote id");
        assert!(html.contains("#r.foot"), "missing return link");
        assert!(html.contains("#f.foot"), "missing footnote link");

        assert!(
            html.find("id=\"r.foot").unwrap() < html.find("#r.foot").unwrap(),
            "return id should be before return link"
        );
        assert!(
            html.find("id=\"f.foot").unwrap() > html.find("#f.foot").unwrap(),
            "footnote id should be after footnote link"
        );
    }
}
