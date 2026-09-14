#!/bin/sh
docker run --rm -v "$PWD:/src" -w /src ubuntu:24.04 sh -c 'apt-get update && apt-get install -y cmake g++ ninja-build && cmake -S . -B build/ubuntu -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build/ubuntu'
