#pragma once

#include "util.hpp"
#include "catalog.hpp"
#include <string>
#include <vector>

/* get all the active tmux sessions */
inline Entries
active_tmux_sessions()
{

  MAKE_CMD(cmd, "tmux", "list-sessions", "-F", "#{session_name}\t#{pane_current_path}");
  CommandResult result = run_command(cmd);
  if (result.exit_code != 0)
  {
    WriteLog("Error listing tmux seesions[{}]", result.exit_code);
    return {};
  }

  Entries output;
  std::istringstream lines{result.output};
  for (std::string line; std::getline(lines, line);)
  {
    if (auto trimmed = trim(line); !trimmed.empty())
    {
      auto tab = line.find('\t');
      std::string name = line.substr(0, tab);
      std::string path;
      if (tab == std::string::npos) { path = home_dir(); }
      else                          { path = line.substr(tab + 1); }

      if (!name.empty())
      {
        output.emplace_back(TmuxEntry{name, path, true});
      }
    }
  }
  return output;
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
execute(const Entry& entry, const Entries& active_entries)
{
  std::string session_name{entry.key()};
  if (!std::ranges::contains(active_entries, entry.key(), &Entry::key))
  {
    auto target_path = home_dir();
    if (entry.kind() == EntryKind::TmuxEntry && !entry.active())
    {
      target_path = fs::path{entry.info()};
    }

    create_session(session_name, target_path.string());

    if (auto* ssh_entry = std::get_if<SshEntry>(&entry.data); ssh_entry)
    { /* in ssh context the name of the action/entry is the name of the host */
      send_keys(session_name, "ssh " + ssh_entry->user + "@" + ssh_entry->hostname);
    }
    else if (entry.kind() == EntryKind::DockerEntry)
    { /* in docker context the name of the action/entry is the container name */
      send_keys(session_name, "docker exec -it " + session_name + " bash");
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

