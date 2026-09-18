#pragma once

#include "catalog.hpp"
#include <string_view>
#include <vector>

struct FuzzyMatch
{
  bool matched{false};
  int score{0};
  std::vector<size_t> indices;
};

[[nodiscard]] FuzzyMatch
fuzzy_match(std::string_view text, std::string_view query, EntryKind kind);

