#!/bin/sh

cmake --build --preset debug -j$(nproc)
cmake --build --preset release -j$(nproc)
docker run --rm -v "$PWD:/src" -w /src ubuntu:24.04 sh -c 'apt-get update && apt-get install -y cmake g++ ninja-build && cmake -S . -B build/ubuntu -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build/ubuntu'

cp -f build/debug/allmux build/allmux-debug
cp -f build/release/allmux build/allmux-release
cp -f build/ubuntu/allmux build/allmux-ubuntu
