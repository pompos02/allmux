#pragma once
#include <charconv>
#include <cmath>
#include <deque>
#include <fstream>
#include <cmath>
#include <ranges>
#include <unordered_map>
#include <algorithm>
#include "util.hpp"

inline constexpr std::int64_t seconds_per_day = 86'400;
inline constexpr std::int64_t retention = 30 * seconds_per_day;
inline constexpr std::size_t maximum_entries = 128;
inline constexpr double decay = 0.0693;
inline fs::path history_file = cache_dir() / "history.tsv";

using HistoryEntries = std::unordered_map<std::string, std::deque<std::int64_t>>;

inline HistoryEntries
load_history()
{
  std::ifstream input{history_file};
  HistoryEntries entries;
  for (std::string line; std::getline(input, line);)
  {
    const auto tab = line.find('\t');
    if (tab == std::string::npos) { continue; }


    // the substr is the entryname(key) of the timestamps(values)
    // takes the reference of the structure an mutate it
    std::deque<std::int64_t>& timestamps = entries[line.substr(0, tab)];

    // the values of the key
    std::string_view values{line.data() + tab + 1, line.size() - tab - 1};

    for (;!values.empty();)
    {
      const auto comma = values.find(',');
      const auto token = values.substr(0, comma);
      int64_t timestamp{};
      const auto [ptr, _] = std::from_chars(token.data(), token.data() + token.size(), timestamp);
      if (ptr == token.data() + token.size()) { timestamps.push_back(timestamp); }
      if (comma == std::string::npos) { break; }
      values.remove_prefix(comma + 1);
    }
  }

  return entries;
}

inline void
record_history(HistoryEntries& entries, std::string_view key)
{
  int64_t timestamp = time_now();

  // the target timestamps to write
  auto& timestamps = entries[std::string{key}];

  // cleanup old timestamps or make space if full
  for (;!timestamps.empty() && (timestamps.front() < timestamp - retention ||
                                 timestamps.size() >= maximum_entries);)
  {
    timestamps.pop_front();
  }
  timestamps.push_back(timestamp);

  std::ofstream buffer{history_file, std::ios::trunc};

  /* Re-create the whole file based on the new state of `entries` */
  for (const auto& [key, timestamps] : entries)
  {
    std::string line = key + '\t';
    for (size_t i = 0; i < timestamps.size(); ++i)
    {
      if (i != 0) { line += ","; }
      line += std::to_string(timestamps[i]);
    }
    buffer << line << '\n';
  }
}

inline int64_t
history_score(HistoryEntries& entries, std::string_view key)
{

  const auto it = entries.find(std::string{key});
  if (it == entries.end()) { return 0; }

  const auto timestamp = time_now();
  double total = 0;
  for (const auto& value : it->second | std::views::reverse)
  {
    if (value < timestamp - retention) { break; }
    const double age = static_cast<double>(timestamp - value) / seconds_per_day;
    total += std::exp(-decay * age);
  }
  return static_cast<int64_t>(std::min(60.0, total * 6.0));
}
