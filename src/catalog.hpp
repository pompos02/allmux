#pragma once

#include "util.hpp"
#include <expected>
#include <optional>
#include <variant>
#include <string>
#include <vector>
#include <filesystem>

inline std::span<const std::string> g_active_sessions{};

struct SshEntry
{
  std::string key{}; // alias
  std::string hostname{};
  std::string user{};
  bool        active{false};
};

struct DockerEntry
{
  std::string key{}; // container name
  bool        running{false};
  bool        active{false};
};

struct TmuxEntry
{
  std::string key{}; // session name for active or full path
  std::string path{};
  bool        active{false};
};

/* should match the indices to the `Data` varient below */
enum class EntryKind
{
  SshEntry,     // 0
  DockerEntry,  // 1
  TmuxEntry     // 2
};

struct Entry
{
  using Data = std::variant<
    SshEntry,     // 0
    DockerEntry,  // 1
    TmuxEntry     // 2
  >;

  Entry(SshEntry entry)    : data(std::move(entry)) {}
  Entry(DockerEntry entry) : data(std::move(entry)) {}
  Entry(TmuxEntry entry)   : data(std::move(entry)) {}

  std::string_view key() const
  {
    return std::visit([](const auto &value) -> std::string_view {
        return value.key;
    }, data);
  }
  EntryKind kind() const { return static_cast<EntryKind>(data.index()); }
  bool active() const { return std::visit([](const auto& value) { return value.active; }, data); }

  Data data;
};

struct Action
{
  EntryKind                  kind;
  std::string                name;
  std::optional<std::string> path;
};

using Entries = std::vector<Entry>;

std::expected<Entries, std::string>
ssh_entries(const fs::path &confing_path = home_dir() / ".ssh/config");

std::expected<Entries, std::string>
docker_entries();

std::expected<Entries, std::string>
tmux_entries();
