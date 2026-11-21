Each shell gets its own generator's script; only the opening lines are pinned, since the rest is clap_complete's output.

```console
$ little_boxes completions bash
_little_boxes() {
...

```

```console
$ little_boxes completions elvish

use builtin;
use str;
...

```

```console
$ little_boxes completions fish
# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_little_boxes_global_optspecs
...

```

```console
$ little_boxes completions nushell
module completions {
...

```

```console
$ little_boxes completions powershell

using namespace System.Management.Automation
...

```

```console
$ little_boxes completions zsh
#compdef little_boxes
...

```

```console
$ little_boxes completions tcsh
? 2
error: invalid value 'tcsh' for '<SHELL>'
  [possible values: bash, elvish, fish, nushell, powershell, zsh]

  tip: a similar value exists: 'zsh'

For more information, try '--help'.

```

The man page documents the options and both subcommands.

```console
$ little_boxes man
...
.TH little_boxes 1 [..]
.SH NAME
...
.SH SUBCOMMANDS
...

```

The subcommands print packaging files, so they refuse the options that draw a box.

```console
$ little_boxes --title demo man
? 2
error: the subcommand 'man' cannot be used with '--title <TITLE>'

Usage: little_boxes [OPTIONS]
       little_boxes <COMMAND>

For more information, try '--help'.

```
