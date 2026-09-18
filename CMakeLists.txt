cmake_minimum_required(VERSION 3.25)

project(allmux VERSION 0.1.3 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

add_executable(allmux
  vendor/ftxui.cpp
  src/catalog.cpp
  src/fuzzy.cpp
  src/application.cpp
)
target_include_directories(allmux PRIVATE vendor)
target_compile_options(allmux PRIVATE -Wall -Wextra -Wpedantic)
