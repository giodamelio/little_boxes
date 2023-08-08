```console
$ little_boxes --file ./hello.txt
┏━━━━━━━━━━━━━━┓
┃ Hello World! ┃
┗━━━━━━━━━━━━━━┛

```

```console
$ little_boxes --file ./does_not_exist.txt
? 1
Error: Failed to open file "./does_not_exist.txt"

```

```console
$ little_boxes --file ./hello.txt --title "Hi"
┏━┫ Hi ┣━━━━━━━┓
┃ Hello World! ┃
┗━━━━━━━━━━━━━━┛

```
