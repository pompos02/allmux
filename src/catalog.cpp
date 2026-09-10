#include <expected>
#include <fstream>
#include <sstream>
#include <algorithm>
#include <fstream>
#include <sstream>
#include <set>
#include "catalog.hpp"
#include "util.hpp"

inline bool
is_active(std::string_view s)
{
  return std::ranges::contains(g_active_sessions, s);
}

/* pasrse ~/.ssh/config and return the hosts */
std::expected<Entries, std::string>
ssh_entries(const fs::path &ssh_config_path)
{
  std::fstream input{ssh_config_path};
  if (!input) return std::unexpected("Error opening: " + ssh_config_path.string());

  Entries entries;
  std::vector<size_t> current_hosts;
  for (std::string raw; std::getline(input, raw);)
  {
    const auto line = trim(raw);
    if (line.empty() || line[0] == '#') { continue; }

    std::istringstream fields{line};
    std::string key;
    fields >> key;
    to_lower_inplace(key);
    
    if (key == "host")
    {
      current_hosts.clear();
      for (std::string alias; fields >> alias;)
      {
        if (alias[0] == '!' || alias.contains('*') || alias.contains('?')) { continue; };
        entries.push_back(SshEntry{ .key = alias, .active = is_active(alias)});
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
std::expected<Entries, std::string>
docker_entries()
  
{
  MAKE_CCMD(args, "docker", "ps", "-a", "--format", "{{.Names}}\t{{.Status}}");
  CommandResult result = run_command(args);
  if (result.exit_code != 0) { return std::unexpected("Error on container extraction"); }

  Entries entries;
  std::istringstream lines{result.output};
  for (std::string line; std::getline(lines, line);)
  {
    const auto seperator = line.find('\t');
    if (seperator == std::string::npos) { continue; }

    auto name = line.substr(0, seperator);
    entries.push_back(DockerEntry{.key = name,
                                  .running = line.substr(seperator + 1).starts_with("Up"),
                                  .active = is_active(name)});
  }

  return entries;
}


/* Parse the .allmux config file and get the tmux paths */
std::expected<Entries, std::string>
tmux_entries()
{
  const auto roots_file = config_dir() / ".allmux";
  std::ifstream input{roots_file};

  Entries entries;
  std::set<fs::path> seen;
  std::set<std::string> names;

  for (std::string line; std::getline(input, line);)
  {
    const auto root = home_dir() / trim(line);
    std::error_code error;
    if (!fs::is_directory(root, error)) { continue; }

    for (const auto& entry : fs::directory_iterator{root})
    {
      const auto fname = entry.path().filename().string();
      if (fname.starts_with('.') || !entry.is_directory() ||
          !seen.insert(entry.path()).second || !names.insert(fname).second) { continue; }

      entries.push_back(TmuxEntry{.key = fname,
                                  .path = entry.path().string(),
                                  .active = is_active(fname)});
    }

  }
  return entries;
}

