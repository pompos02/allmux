#pragma once

#include "util.hpp"
#include <optional>
#include <variant>
#include <string>
#include <vector>
#include <filesystem>

struct SshEntry
{
  std::string key{};      // alias
  std::string hostname{};
  std::string user{};
  bool        active{false};
};

struct DockerEntry
{
  std::string key{}; // container name
  bool        active{false};
};

struct TmuxEntry
{
  std::string key{}; // session name
  std::string path{};
  bool        active{false};
};

/* should match the indices to the `Data` varient below 
 * this is also used for ranking, higher index has priority */
enum class EntryKind
{
  SshEntry,     // 0
  DockerEntry,  // 1
  TmuxEntry,    // 2

  Count,        // keep last
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
  Entry(DockerEntry entry, std::string extra)
      : data(std::move(entry)), extra(std::move(extra)) { }
  Entry(TmuxEntry entry)   : data(std::move(entry)) {}

  std::string_view key() const
  {
    return std::visit([](const auto &value) -> std::string_view {
        return value.key;
    }, data);
  }
  EntryKind kind() const { return static_cast<EntryKind>(data.index()); }
  bool active() const { return std::visit([](const auto& value) { return value.active; }, data); }
  std::string_view info() const
  {
    return std::visit([](const auto& value) -> std::string_view{
      using T = std::remove_cvref_t<decltype(value)>;
      if constexpr (std::same_as<T, SshEntry>)    { return value.hostname; }
      if constexpr (std::same_as<T, DockerEntry>) { return value.key; }
      if constexpr (std::same_as<T, TmuxEntry>)   { return value.path; }
    }, data);
  }

  std::string_view display() const
  { /* always display info except active tmux sessions, and ssh sessions */
    const auto entry_kind = kind();
    if ((entry_kind == EntryKind::TmuxEntry && active()) ||
         entry_kind == EntryKind::SshEntry)
    {
      return key();
    }
    return info();
  }

  Data data;
  std::string extra{};
};

struct Action
{
  EntryKind                  kind;
  std::string                name;
  std::optional<std::string> path;
};

using Entries = std::vector<Entry>;

Entries ssh_entries(const Entries& active_entries, const fs::path &confing_path = home_dir() / ".ssh/config");

Entries docker_entries(const Entries& active_entries);

Entries tmux_entries(const Entries& active_entries);
