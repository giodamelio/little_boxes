#!/bin/bash
cargo build
help2man --no-info --name "Adds boxes around stdin. Optionally adds a title." ./target/debug/little_boxes > little_boxes.1
