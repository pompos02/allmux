#include <fstream>
#include <sstream>
#include <algorithm>
#include <set>
#include "catalog.hpp"
#include "util.hpp"

inline bool
is_active(const Entries& active_entries, std::string_view s)
{
  return std::ranges::contains(active_entries, s, &Entry::key);
}

/* pasrse ~/.ssh/config and return the hosts */
Entries
ssh_entries(const Entries& active_entries, const fs::path &ssh_config_path)
{
  std::fstream input{ssh_config_path};
  if (!input) { WriteLog("Error opening: {}", ssh_config_path.string()); }

  Entries entries;
  std::vector<size_t> current_hosts;
  for (std::string raw; std::getline(input, raw);)
  {
    const auto line = trim(raw);
    if (line.empty() || line[0] == '#') { continue; }

    std::istringstream fields{std::string{line}};
    std::string key;
    fields >> key;
    std::ranges::transform(key, key.begin(), [](unsigned char ch) {
      return std::tolower(ch);
    });

    if (key == "host")
    {
      current_hosts.clear();
      for (std::string alias; fields >> alias;)
      {
        if (alias[0] == '!' || alias.contains('*') || alias.contains('?')) { continue; };
        entries.push_back(SshEntry{ .key = alias, .active = is_active(active_entries, alias)});
        current_hosts.push_back(entries.size() - 1);
      }
    }
    if (current_hosts.empty())
    {
      continue;
    }

    std::string value;
    fields >> value;
    if (key == "hostname")
    {
      for (const auto idx : current_hosts)
      {
        std::get<SshEntry>(entries[idx].data).hostname = value;
        entries[idx].extra = value;
      }
    }
    else if (key == "user")
    {
      for (const auto idx : current_hosts)
      {
        std::get<SshEntry>(entries[idx].data).user = value;
      }
    }
  }

  return entries;
}

/* get all docker containers (docker ps -a)*/
Entries
docker_entries(const Entries& active_entries)

{
  MAKE_CMD(args, "docker", "ps", "-a", "--format", "{{.Names}}\t{{.Status}}");
  CommandResult result = run_command(args);
  if (result.exit_code != 0) { WriteLog("Error on container extraction: {}", result.output); }

  Entries entries;
  std::istringstream lines{result.output};
  for (std::string line; std::getline(lines, line);)
  {
    const auto seperator = line.find('\t');
    if (seperator == std::string::npos) { continue; }

    auto name = line.substr(0, seperator);
    auto extra = line.substr(seperator + 1).starts_with("Up") ? "running" : "stopped";
    entries.emplace_back(DockerEntry{name, is_active(active_entries, name)}, extra);
  }

  return entries;
}


/* Parse the .allmux config file and get the tmux paths */
Entries
tmux_entries(const Entries& active_entries)
{
  const auto roots_file = config_dir() / ".allmux";
  std::ifstream input{roots_file};

  Entries entries;
  std::set<fs::path> seen;
  std::set<std::string> names;

  for (std::string line; std::getline(input, line);)
  {
    const auto root = home_dir() / fs::path{trim(line)};
    std::error_code error;
    if (!fs::is_directory(root, error)) { continue; }

    for (const auto& entry : fs::directory_iterator{root})
    {
      const auto fname = entry.path().filename().string();
      if (fname.starts_with('.') || !entry.is_directory() ||
          !seen.insert(entry.path()).second || !names.insert(fname).second) { continue; }

      entries.push_back(TmuxEntry{.key = fname,
          .path = entry.path().string(),
          .active = is_active(active_entries, fname)});
    }

  }
  return entries;
}
