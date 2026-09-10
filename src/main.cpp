#include "catalog.hpp"
#include <exception>
#include <print>

int
main()
{
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
