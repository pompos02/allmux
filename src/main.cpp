#include "catalog.hpp"
#include "tmux.hpp"
#include <print>

int
main()
{
  /* init global active tmux sessions */
  g_active_sessions = active_tmux_sessions();

  auto entries = ssh_entries();
  entries = docker_entries();
  entries = tmux_entries();

  if (!entries)
  {
    std::println("{}", entries.error());
  }
  else
  {
    for (const auto& entry : *entries)
    {
      std::println("name = {}", entry.key());
    }
  }
  std::println("Hello World");
  return 0;
}
