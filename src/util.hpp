#pragma once

#include <cctype>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <spawn.h>
#include <string>
#include <string_view>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>
#include <algorithm>
#include <chrono>

#include "logger.hpp"

#define MAKE_CMD(name, ...) \
    std::string_view name[] = { __VA_ARGS__ }


namespace fs = std::filesystem;

inline std::string_view
trim(std::string_view value)
{
  while (!value.empty() && std::isspace(static_cast<unsigned char>(value.front()))) { value.remove_prefix(1); }
  while (!value.empty() && std::isspace(static_cast<unsigned char>(value.back()))) { value.remove_suffix(1); }
  return value;
}

inline fs::path
home_dir()
{
  if (const char *home = std::getenv("HOME")) { return home; }
  else                                        { return "."; }
}

inline fs::path
config_dir()
{
  if (const char *dir = std::getenv("XDG_CONFIG_HOME")) { return dir; }
  else                                                  { return home_dir() / ".config"; }
}

inline fs::path
cache_dir()
{
  fs::path path;
  if (const char *dir = std::getenv("XDG_CACHE_HOME"))  { path = fs::path{dir} / "allmux"; }
  else                                                  { path = home_dir() / ".cache/allmux"; }

  fs::create_directories(path);
  return path;
}

inline fs::path
log_file()
{
  return cache_dir() / "allmux.dmp";
}

struct CommandResult
{
  int         exit_code{1};
  std::string output;
};

[[nodiscard]] inline CommandResult
run_command(std::span<const std::string_view> args)
{
  if (args.empty()) { return {1, "Empty command"}; }
  int pipe_fd[2];
  if (pipe(pipe_fd) == -1) { return {-1, std::strerror(errno)}; }

  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_adddup2(&actions, pipe_fd[1], STDOUT_FILENO);
  posix_spawn_file_actions_adddup2(&actions, pipe_fd[1], STDERR_FILENO);
  posix_spawn_file_actions_addclose(&actions, pipe_fd[0]);
  posix_spawn_file_actions_addclose(&actions, pipe_fd[1]);

  std::vector<std::string> owned_args(args.begin(), args.end());
  std::vector<char *> argv;
  argv.reserve(owned_args.size() + 1);
  for (std::string &arg : owned_args) { argv.emplace_back(arg.data()); }
  argv.push_back(nullptr);

  pid_t pid{};
  const int spawn_error = posix_spawnp(&pid, argv.front(), &actions, nullptr, argv.data(), environ);
  posix_spawn_file_actions_destroy(&actions);
  close(pipe_fd[1]);

  if (spawn_error != 0)
  {
    close(pipe_fd[0]);
    return {spawn_error, strerror(spawn_error)};
  }

  CommandResult result;
  std::array<char, 4096> buffer{};
  for (;;)
  {
    const auto count = read(pipe_fd[0], buffer.data(), buffer.size());
    if (count > 0)                          { result.output.append(buffer.data(), static_cast<std::size_t>(count)); }
    else if (count == -1 && errno == EINTR) { continue; }
    else                                    { break; }
  }

  close(pipe_fd[0]);
  int status{};
  pid_t waited;

  do { waited = waitpid(pid, &status, 0); } while (waited == -1 && errno == EINTR);

  if (waited == -1)           { result.exit_code = -1; }
  else if (WIFEXITED(status)) { result.exit_code = WEXITSTATUS(status); }
  else                        { result.exit_code = 1; }
  return result;
}


inline int64_t
time_now()
{
  using namespace std::chrono;
  return duration_cast<seconds>(system_clock::now().time_since_epoch()).count();

}

inline bool
is_dark_theme()
{
  std::ifstream file{cache_dir() / "theme"};
  std::string variant;
  std::getline(file, variant);
  if (trim(variant) == "light") { return false; }
  else                    { return true ; }
}

inline std::string
toggle_theme()
{
  auto file = cache_dir() / "theme";
  auto theme = is_dark_theme() ? "light" : "dark";
  std::ofstream buffer{file, std::ios::trunc};
  if (!buffer) { WriteLog("Error opening {}", file.string()); }
  buffer << theme;
  return theme;
}
