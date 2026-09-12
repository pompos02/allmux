#pragma once

#include "util.hpp"
#include "catalog.hpp"
#include <string>
#include <vector>

/* get all the active tmux sessions */
inline std::vector<std::string>
active_tmux_sessions()
{

  MAKE_CMD(cmd, "tmux", "list-sessions", "-F", "#{session_name}");
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

/* create a new tmux session, we  */
inline void
create_session(std::string& o_name, std::string_view path = {})
{
  /* tmux doesn't behave good if we have '.' in the name */
  for (char& ch : o_name) { if (ch == '.') ch = '_'; }

  const auto directory = path.empty() ? home_dir().string() : std::string(path);
  MAKE_CMD(cmd, "tmux", "new-session", "-d", "-P", "-F",
           "#{session_name}:#{window_index}.#{pane_index}", "-s", o_name, "-c", directory);
  CommandResult result = run_command(cmd);
  if (result.exit_code != 0)
  {
    WriteLog("Error while creating session '{}' in directory '{}'", o_name, directory);
  }

}

/* send keys(command) to a tmux session */
inline void send_keys(std::string_view target, std::string_view command)
{
  MAKE_CMD(cmd, "tmux", "send-keys", "-t", target, command, "C-m");
  CommandResult result = run_command(cmd);
  if (result.exit_code != 0)
  {
    WriteLog("Error sending '{}' to session '{}'", command, target);
  }
}

/* switch to a tmux session */
inline void switch_to(std::string_view target)
{
  MAKE_CMD(cmd, "tmux", "switch-client", "-t", target);
  CommandResult result = run_command(cmd);
  if (result.exit_code)
  {
    WriteLog("Error switching to tmux session:{}", target);
  }
}

/* execute entry specific action called when an entry is pressed */
inline void
execute(Entry& entry)
{
  std::string session_name{entry.key()};
  if(!std::ranges::contains(g_active_sessions, entry.key()))
  {
    auto target_path = home_dir();
    if (entry.kind() == EntryKind::TmuxEntry && !entry.active())
    {
      target_path = fs::path{entry.key()};
    }

    create_session(session_name, target_path.string());

    if (entry.kind() == EntryKind::SshEntry)
    { /* in ssh context the name of the action/entry is the name of the host */
      send_keys(session_name, "ssh " + session_name);
    }
    else if (entry.kind() == EntryKind::DockerEntry)
    { /* in docker context the name of the action/entry is the container name */
      send_keys(session_name, "docker exect -it " + session_name + " bash");
    }
  }
  switch_to(session_name);
}

/* use tmux copy buffer to copy the hovering entry */
inline bool
copy_info(std::string_view value)
{
  MAKE_CMD(cmd, "tmux", "set-buffer", "-w", "--", value);
  return run_command(cmd).exit_code == 0;
}


