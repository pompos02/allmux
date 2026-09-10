#pragma once

#include <span>
#include <string_view>

struct FuzzyMatch {
  bool matched{false};
  int score{0};
  std::span<const size_t> indices{};
};

[[nodiscard]] FuzzyMatch
fuzzy_match(std::string_view text, std::string_view query, std::span<size_t> matched_indices);
