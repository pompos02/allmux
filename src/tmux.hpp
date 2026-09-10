#pragma once

#include "util.hpp"
#include <string>
#include <vector>

/* get all the active tmux sessions */
inline std::vector<std::string>
active_tmux_sessions()
{

  MAKE_CCMD(cmd, "tmux", "list-sessions", "-F", "#{session_name}");
  CommandResult result = run_command(cmd);
  if (result.exit_code != 0)
  {
    WriteLog("Error listing tmux seesions[{}]", result.exit_code);
    return {};
  }

  std::vector<std::string> sessions;
  std::istringstream lines{result.output};
  for (std::string line; std::getline(lines, line);)
  {
    if (auto trimmed = trim(line); !trimmed.empty())
      sessions.push_back(std::move(trimmed));
  }
  return sessions;
}

/* create a new tmux session */
inline void create_session(std::string_view name, std::string_view path = {})
{
  const auto directory = path.empty() ? home_dir().string() : std::string(path);
  MAKE_CMD(cmd, "tmux", "new-session", "-d", "-P", "-F",
           "#{session_name}:#{window_index}.#{pane_index}", "-s", name, "-c",
           directory);
}
