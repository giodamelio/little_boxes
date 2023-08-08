```console
$ little_boxes --file ./hello.txt
┏━━━━━━━━━━━━━━┓
┃ Hello World! ┃
┗━━━━━━━━━━━━━━┛

```

```console
$ little_boxes --file ./does_not_exist.txt
? 1
Error: No such file or directory (os error 2)

```

```console
$ little_boxes --file ./hello.txt --title "Hi"
┏━┫ Hi ┣━━━━━━━┓
┃ Hello World! ┃
┗━━━━━━━━━━━━━━┛

```
