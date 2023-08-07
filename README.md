# Little Boxes
[![Rust](https://github.com/giodamelio/little_boxes/actions/workflows/rust.yml/badge.svg)](https://github.com/giodamelio/little_boxes/actions/workflows/rust.yml) [![Crates.io Version](https://img.shields.io/crates/v/little_boxes.svg?style=flat-square)](https://crates.io/crates/little_boxes) [![License](https://img.shields.io/crates/l/little_boxes.svg?style=flat-square)](https://github.com/giodamelio/little_boxes/blob/master/LICENSE) 

Adds boxes around stdin. Optionally adds a title.

![preview](preview.png)

# Installation

```sh
# From crates.io
$ cargo install little_boxes

# Try it out with Nix flakes
$ echo "Hello World" | nix run github:giodamelio/little_boxes
```

# Usage

```
little_boxes [options]

Options:
  -c, --charset <charset>    The charset to draw the box with [default: thick]
                             Available charsets: thick, thin, double, box, rounded and dot
  -t, --title <title>        Add a title to the box
  -a, --all                  Compare all the styles
  -h, --help                 Shows this help
  -v, --version              Show version
```
