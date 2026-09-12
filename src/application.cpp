#include "fuzzy.hpp"
#include "history.hpp"
#include "util.hpp"
#include "catalog.hpp"

#include <algorithm>
#include <ranges>
#include <cctype>
#include <ftxui/component/component.hpp>
#include <ftxui/component/event.hpp>
#include <ftxui/component/screen_interactive.hpp>
#include <ftxui/dom/elements.hpp>
#include <memory>
#include <variant>

#define IF_VARIANT(Type, name, variant) if (const auto* name = std::get_if<Type>(&(variant)))

using namespace ftxui;

struct Match
{
  size_t index;                 // Position in entries
  int score;                    // Fuzzy score + hist bonus
  std::vector<size_t> indices;  // Highlighted chars position
};

std::vector<Match>
matches(Entries& entries, std::string_view query)
{
  //TODO: see if this should actually be calles here
  HistoryEntries hentries = load_history();
  std::vector<Match> result;
  for (size_t i = 0; i < entries.size(); ++i)
  {
    std::string_view text = entries[i].key();
    std::vector<size_t> buffer(text.size());
    size_t fscore = fuzzy_match(text, query, buffer);
    if (!fscore) { continue; }
    int64_t hscore = history_score(hentries, text);
    int64_t score = fscore + static_cast<int64_t>(fscore * hscore / 60);
    result.emplace_back(i, score, buffer);
  }

  // priority sorting
  std::ranges::sort(result, [&](Match& left, Match& right){
      auto& a = entries[left.index];
      auto& b = entries[right.index];
      if (left.score != right.score)  { return left.score > right.score; }
      if (a.active() != b.active())   { return a.active() > b.active(); }
      if (a.kind() != a.kind())       { return a.kind() > a.kind(); }
      return left.index > right.index;
  });

  return result;
}


Entry
run()
{
  // auto app = ftxui::App::Fullscreen();
  // auto component = CatchEvent();
  // app.Loop(void);
  // auto renderer = Renderer([&](){
  //   const auto filtered = matches
  // });
}

