cmake_minimum_required(VERSION 3.25)

project(allmux VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 26)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

add_subdirectory(vendor/ftxui)

add_executable(allmux
  src/catalog.cpp
  src/fuzzy.cpp
  src/application.cpp
  src/main.cpp
)
target_link_libraries(allmux PRIVATE
  ftxui::component
  ftxui::dom
  ftxui::screen
)
target_compile_options(allmux PRIVATE -Wall -Wextra -Wpedantic)
