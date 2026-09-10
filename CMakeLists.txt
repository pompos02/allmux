cmake_minimum_required(VERSION 3.25)

project(allmux VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 26)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

include(FetchContent)
FetchContent_Declare(
  ftxui
  GIT_REPOSITORY https://github.com/ArthurSonzogni/FTXUI.git
  GIT_TAG v6.1.9
)
FetchContent_MakeAvailable(ftxui)

add_library(allmux_lib
  src/catalog.cpp
  src/fuzzy.cpp
  # src/history.cpp
  # src/model.cpp
  # src/process.cpp
  # src/tmux.cpp
  # src/util.cpp
)
target_compile_options(allmux_lib PRIVATE -Wall -Wextra -Wpedantic)

add_executable(allmux src/main.cpp)
target_link_libraries(allmux PRIVATE
  allmux_lib
  ftxui::component
  ftxui::dom
  ftxui::screen
)
target_compile_options(allmux PRIVATE -Wall -Wextra -Wpedantic)
