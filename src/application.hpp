#pragma once

#include "catalog.hpp"

/* global vector of Entry that holds all the active tmux sessions */
inline std::vector<Entry> g_entries{};

Entry run();

