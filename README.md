# Pagong ![pagong's logo](logo.svg)

> *Boring-simple Static-Site-Generator*

You want a website but writing HTML by hand is awful. I get it. But that's no problem! Write markdown at your leisure, run `pagong` and get your nice HTML lightning fast, ready to be uploaded to your hosting service!

## Getting started

### Installation

Install `pagong` by running the following command on a terminal:

```sh
cargo install --git https://github.com/expectocode/pagong
```

Then use it in your blog's root folder:

```sh
pagong
```

It's that simple!

### Blog structure

For `pagong` to do anything useful, you need to have some entries for your blog. These should be written in markdown and saved in the `content/` directory as `.md` files. For example:

```
myblog/
└── content/
    └── hello-world.md
```

Running `pagong` while inside `myblog` will create the following `dist/` folder, and the tree of your blog now looks like this:

```
myblog/
├── content/
│   └── hello-world.md
└── dist/
    └── hello-world/
        └── index.html
```

Now you can move the contents of `dist/` to wherever you host your site and enjoy it.

## Customization

### Metadata

Your `.md` files may contain the following syntax at the very top:

````
```meta
key = value
```

**Markdown** content follows as usual…
````

The code-block with `meta` lang must be the first markdown element in the file. The supported keys are:

* `title`: Post title (e.g. "Hello, world!"). If not specified, the first heading in the document is considered the title. If there is no first heading, the file name is used.
* `date`: Published date, `YYYY-MM-DD` (Year, Month, Day) format (e.g. "2020-02-20"). If not specified, the file's creation date will be used. If it cannot be fetched, the current date will be used.
* `updated`: Updated date, `YYYY-MM-DD` format. If not specified, the file's modification date will be used. If it cannot be fetched, `date` will be used.
* `category`: Category where the post belongs to (e.g. "computing"). If not specified, the parent folder name will be used (e.g. "blog").
* `tags`: Comma-separated list of tags (e.g. "rust, ssg"). If not specified, an empty list of tags is produced.
* `template`: Path to the HTML file to be used as the template for this file, UNIX-style path, relative wherever the current file is (e.g. "/_blog.html" or "../_template.html").

Any other key will be ignored by `pagong`, but may be used for your own needs.

### CSS

Any `.css` file will be copied to `dist/`, and any `.md` will load all the `.css` files in the same directory or above.

```
myblog/
└── content/
    ├── index.md
    ├── sitewide.css
    └── blog/
        ├── hello-world.md
        └── blogwide.css
```

The HTML generated for `index.md` will use `sitewide.css`, and the HTML generated for `hello-world.md` will include `sitewide.css` and then `blogwide.css`.

### HTML

Any `.html` file will be copied to `dist/` as-is, with the exception files mentioned in the metadata of any of the `.md` files. If `hello-world.md` includes `template = /templates/base.html`, then `base.html` won't be copied over as-is, and instead, it will be used as a template. You're encouraged to follow your own convention as to where to place the templates or how they should be named.

HTML files used as templates offer some very minimal "pre-processor" rules, which are HTML comments with a few adornments:

```html
This comment will tell pagong to insert the generated HTML in this spot:
<!--P/ CONTENTS /P-->

This comment will tell pagong to insert references to any CSS files in this spot:
<!--P/ CSS /P-->

This comment will tell pagong to automatically generate a Table of Contents for the current page (based on Markdown headings). You may optionally set the maximum depth:
<!--P/ TOC /P-->
<!--P/ TOC 3 /P-->

This comment will tell pagong to automatically generate a list of files in the given path (relative to the current markdown file):
<!--P/ LIST path /P-->

This comment will get replaced with whatever was put in the specified metadata key (in this example, the title):
<!--P/ META title /P-->

This comment will get replaced with the contents of whatever path is specified (relative to the current markdown file). HTML files won't be escaped, but everything else will:
<!--P/ INCLUDE path /P-->
```

When replacing the "pre-processor" rules, the code will look exactly for the strings `<!--P/` and `/P-->`, so make sure to not introduce spaces in-between. If any of the values to the pre-processor rules contain spaces, surround them in double-quotes (`"`). The only escape sequences allowed inside double-quotes are `\"` in order to escape a quote, and `\\` in order to escape the backslash character.

A default [`template.html`] file is embedded withing `pagong` itself. It will be used when no other template file is specified, in order to generate valid HTML5 (your HTML needs a body, after all).

[`template.html`]: https://github.com/Lonami/pagong/blob/master/template.html

### Feed

Any `.atom` file will be copied to `dist/`, but its root `feed` tag will be filled with `entry` tags automatically. Here's a basic `.atom` file which would do the trick (and you're free to remove the `generator` tag):

```xml
<feed xml:lang="en">
    <title>Example's Blog</title>
    <link href="https://example.com/blog/"/>
    <generator uri="https://github.com/expectocode/pagong">pagong with atom_syndication</generator>
</feed>
```

### Media

Any other file will be copied over without any processing done to it, with the same path and name as it existed in the `content/` directory.

## Contributing

The number of features this project offers is intentionally small. Issues and pull requests regarding bugs or possible enhancements are welcome. New features or substantial changes must first be discussed in the issues section. Pull requests of new features without previous discussion will be rejected, but you are welcome to maintain your own fork.

## License

Pagong is licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
