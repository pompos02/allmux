#pragma once

#include <cctype>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <spawn.h>
#include <string>
#include <string_view>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>
#include <algorithm>

#include <string_view>
#include "logger.hpp"

#define MAKE_CCMD(name, ...) \
    constexpr std::string_view name[] = { __VA_ARGS__ }

#define MAKE_CMD(name, ...) \
    std::string_view name[] = { __VA_ARGS__ }


namespace fs = std::filesystem;

inline std::string
trim(std::string_view value)
{
  const char *begin = value.data();
  const char *end = begin + value.size();
  for (; begin != end && std::isspace(*begin); ++begin) { }
  for (; end != begin && std::isspace(*(end - 1)); --end) { }
  return std::string(begin, end);
}

inline std::string_view
trim_view(std::string_view value)
{
  const char *begin = value.data();
  const char *end = begin + value.size();
  for (; begin != end && std::isspace(*begin); ++begin) { }
  for (; end != begin && std::isspace(*(end - 1)); --end) { }
  return std::string_view(begin, end);
}

/**
 * @brief Trims leading and trailing whitespace from a view, starting at a given
 * offset.
 *
 * @param value The input string view to be trimmed.
 * @param start The offset index from which to begin trimming.
 * @return std::string_view A view of the trimmed string, or an empty view if
 * `start >= value.size()`.
 */
inline std::string_view
get_word(std::string_view value, size_t start, size_t &o_end)
{
  if (start >= value.size()) return {};
  size_t begin{start};
  for (; begin < value.size() && std::isspace(value[begin]); ++begin) { }
  size_t end{begin};
  for (; end < value.size() && !std::isspace(value[end]); ++end) { }

  o_end = end;
  return value.substr(begin, end - begin);
}

inline void
to_lower_inplace(std::string& s)
{
  std::ranges::transform(s, s.begin(), [](unsigned char c) {
      return std::tolower(c);
  });
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
default_config_path()
{
  return config_dir() / ".allmux";
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
  auto cache = cache_dir();
  return cache / "allmux.dmp";
}

inline std::string
shell_quote(std::string_view value)
{
  std::string result{"'"};
  for (const char ch : value)
  {
    if (ch == '\'') { result += "'\\''"; }
    else            { result += ch; }
  }
  result += '\'';
  return result;
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

