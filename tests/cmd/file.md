```console
$ little_boxes --file ./hello.txt
┏━━━━━━━━━━━━━━┓
┃ Hello World! ┃
┗━━━━━━━━━━━━━━┛

```

```console
$ little_boxes --file ./does_not_exist.txt
? 1
Error: File "./does_not_exist.txt" does not exist

```

```console
$ little_boxes --file /dev/null
┏━━┓
┗━━┛

```

```console
$ little_boxes --file ./empty_dir
? 1
Error: "./empty_dir" is a directory, it must be a file

```

```console
$ little_boxes --file ./hello.txt --title "Hi"
┏━┫ Hi ┣━━━━━━━┓
┃ Hello World! ┃
┗━━━━━━━━━━━━━━┛

```
