# Pagong ![pagong's logo](logo.svg)

> *Write markdown, get a static site*

This project is a bit of a work in progress. Expect more nice things to come!

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
    ├── hello-world.md
    └── style.css
```

Running `pagong` while inside `myblog` will create the following `dist/` folder, and the tree of your blog now looks like this:

```
myblog/
├── content/
│   ├── hello-world.md
│   └── style.css
└── dist/
    ├── atom.xml
    ├── css/
    │   └── style.css
    ├── hello-world/
    │   └── index.html
    └── index.html
```

Now you can move the contents of `dist/` to wherever you host your site and enjoy it.

### Styling

We provide a [default `style.css`](https://raw.githubusercontent.com/expectocode/pagong/master/style.css) that you need to copy into your `content/` folder if you want your blog to look pretty. This is completely optional, and you can also write your own if you want.

## Customization

### Assets

If you want to embed assets into your blog entries, create a directory for the entry instead, and put the text contents inside `post.md`. Then, include any assets you want in the same folder:

```
myblog/
└── content
    └── hello-world
        ├── asset.jpg
        └── post.md
```

Inside `post.md`, you can simply refer to `asset.jpg` to make use of it:

```md
# Hello, world!

![A beautiful asset.](asset.jpg)
```

### Post metadata

Post metadata is included within the `.md` itself as a fenced block with the `"meta"` language at the beginning of the post's content. This code block won't be directly visible in the generated HTML, but will instruct `pagong` how to do certain things. For example, in `post.md`:

<pre>
```meta
created: 2020-02-20
```

# My blog post

Welcome!
</pre>

The meta definitions are key-value pairs, separated by the `:` character, and the valid keys are:

* `created` or `published`: overrides the creation date of the entry, in `YYYY-mm-dd` format.
* `modified` or `updated`: overrides the date of the last update of the entry, in `YYYY-mm-dd` format.
* `title`: overrides the title of the post.
* `path`: overrides the path of the post (so that the URL can be different from the file name).

### Naming convention

The names for the metadata keys or classes to be used in the CSS should generally be the obvious thing you would expect. Check the source code to be sure :)

## License

Pagong is licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
