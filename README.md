# Little Boxes [![Build Status](https://travis-ci.org/giodamelio/little_boxes.svg?branch=master)](https://travis-ci.org/giodamelio/little_boxes)

Adds boxes around stdin. Optionally adds a title.

![preview](preview.png)

# Installation

```sh
# From crates.io
$ cargo install little_boxes
# From the AUR
$ packer -S little_boxes
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
