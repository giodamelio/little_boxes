```console
$ little_boxes --help
Adds boxes around stdin. Optionally adds a title

Usage: little_boxes [OPTIONS]

Options:
  -t, --title <TITLE>        Add a title to the box
  -c, --charset <CHARSET>    The charset to draw the box with [default: thick] [possible values: thick, thin, double, box, rounded, dot, ascii]
  -f, --file <FILE>          Read input from a file instead of stdin
      --all                  Compare all charsets
      --border-color <SPEC>  Color the border: a name like red or bright-blue, a 256-color index 0-255, or #rrggbb
      --color <WHEN>         When to color the border [default: auto] [possible values: auto, always, never]
  -h, --help                 Print help
  -V, --version              Print version

```
