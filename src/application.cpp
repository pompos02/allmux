#include "fuzzy.hpp"
#include "history.hpp"
#include "tmux.hpp"
#include "catalog.hpp"

#include <algorithm>
#include <cctype>
#include <ftxui/component/component.hpp>
#include <ftxui/component/event.hpp>
#include <ftxui/component/screen_interactive.hpp>
#include <ftxui/dom/elements.hpp>
#include <thread>

static int8_t g_loading = static_cast<int8_t>(EntryKind::Count);
using namespace ftxui;

struct Match
{
  size_t index;                 // Position in entries
  int score;                    // Fuzzy score + hist bonus
  std::vector<size_t> indices;  // Highlighted chars position
};

std::string
label(EntryKind kind)
{
  switch (kind)
  {
  case EntryKind::SshEntry:    return " SSH ";
  case EntryKind::DockerEntry: return " DOC ";
  case EntryKind::TmuxEntry:   return " MUX ";
  default: return {};
  }
}

static Color
color_for(EntryKind kind)
{
  switch (kind)
  {
  case EntryKind::SshEntry:    return Color::Cyan;
  case EntryKind::DockerEntry: return Color::Blue;
  case EntryKind::TmuxEntry:   return Color::Green;
  default: return Color::Default;

  }
}

static Color
classify_text_color(std::string_view text)
{
  if      (text == "running") { return Color::Green; }
  else if (text == "stopped") { return Color::Red; }
  else                        { return Color::Blue; }
}

static Color
selection_color()
{
  return is_dark_theme() ? Color::Grey30 : Color::RGB(232, 232, 232);
  
}

static Element
highlighted(std::string_view text, std::span<const size_t> indices, size_t dim_until = 0)
{
  Elements parts;
  for (size_t pos = 0; pos < text.size();)
  {
    bool matched = std::ranges::binary_search(indices, pos);
    bool dimmed = pos < dim_until;
    size_t end = pos + 1;
    while (end < text.size() && std::ranges::binary_search(indices, end) == matched && (end < dim_until) == dimmed)
    {
      ++end;
    }
    
    auto part = ftxui::text(std::string{text.substr(pos, end - pos)});
    if (matched) { part = part | inverted; }
    else if (dimmed) { part = part | dim; }
    parts.push_back(std::move(part));
    pos = end;
  }
  return hbox(std::move(parts));
}

static void
delete_word(std::string& text)
{
  for (;!text.empty() && text.back() == isspace(text.back());text.pop_back()) {}
  for (;!text.empty() && text.back() != isspace(text.back());text.pop_back()) {}
}

std::vector<Match>
matches(Entries& entries, std::string_view query, const HistoryEntries& hentries)
{
  std::vector<Match> result;
  for (size_t i = 0; i < entries.size(); ++i)
  {
    std::string_view text = entries[i].display();
    int64_t hscore = history_score(hentries, entries[i].info());
    if (trim_view(query).empty())
    { /* empty query gives all entries score of 1 */
      result.emplace_back(i, 1 + static_cast<int64_t>(hscore * 0.5), std::vector<size_t>{});
      continue;
    }
    auto fuzzy = fuzzy_match(text, query);
    if (!fuzzy.matched) { continue; }

    int64_t score = fuzzy.score + static_cast<int64_t>(fuzzy.score * hscore / 60);
    result.emplace_back(i, score, std::move(fuzzy.indices));
  }

  // priority sorting
  std::ranges::sort(result, [&](Match& left, Match& right){
      auto& a = entries[left.index];
      auto& b = entries[right.index];
      if (left.score != right.score)  { return left.score > right.score; }
      if (a.active() != b.active())   { return a.active() > b.active(); }
      if (a.kind() != b.kind())       { return a.kind() > b.kind(); }
      return left.index > right.index;
  });

  return result;
}

void
merge_entries(Entries& entries, Entries& incoming_entries)
{
  /* Skip the tmux entries that are active and created my another entry type
   * so we don't have duplicated entryes of <other_type> and Tmux */
  for (const auto& incoming : incoming_entries)
  {
    if (incoming.kind() == EntryKind::TmuxEntry)
    { /* skip the tmux entry if entries with the same name exists */
      auto represented = std::ranges::any_of(entries,[&](const Entry& current){
          return current.kind() != EntryKind::TmuxEntry && incoming.key() == current.key();
      });
      if (represented) { continue; }
    }
    else
    { /* remove the tmux entry that has the same name with the soon to be merged entry */
      std::erase_if(entries,[&](const Entry& current){
          return current.kind() == EntryKind::TmuxEntry && current.key() == incoming.key();
      });
    }
    entries.push_back(incoming);
  }
}

/* define the work of each worker */
template<typename Loader>
static std::jthread
launch_loader(Entries& current_entries, ScreenInteractive& screen, Loader loader)
{
  return std::jthread([&screen, loader = std::move(loader), &current_entries]() mutable {
    auto entries = loader();
    screen.Post([entries = std::move(entries), &current_entries]() mutable {
        merge_entries(current_entries, entries);
        g_loading--;
    });
    screen.PostEvent(Event::Custom);
  });
}

std::jthread
load(Entries& o_entries, ScreenInteractive& screen, const Entries& active_entries)
{
  return std::jthread([&]{
    constexpr auto worker_num = static_cast<size_t>(EntryKind::Count);
    std::array<std::jthread, worker_num> workers;
    workers[0] = launch_loader(o_entries, screen, [&]{ return ssh_entries(active_entries); });
    workers[1] = launch_loader(o_entries, screen, [&]{ return docker_entries(active_entries); });
    workers[2] = launch_loader(o_entries, screen, [&]{ return tmux_entries(active_entries); });
  });


}


void
run()
{
  Entries entries;
  auto app = ftxui::App::Fullscreen();
  const auto active_entries = active_tmux_sessions();
  auto loader = load(entries, app, active_entries);
  HistoryEntries history_entries = load_history();
  std::string s_query{};  // the actual query written
  std::string s_status{}; // status to show on operations
  size_t s_selected{0};      // the selected entry index
  Color selected_color = selection_color();

  /* Renderer Implementation */
  auto renderer = Renderer([&](){
    const auto filtered = matches(entries, s_query, history_entries);
    Elements rows;
    for (size_t pos = 0; pos < filtered.size(); ++pos)
    {
      const auto& match = filtered[pos];
      const auto& entry = entries[match.index];
      const bool selected =  pos == s_selected;
      const auto style = selected ? bgcolor(selected_color) | bold | focus : nothing;
      size_t dim_until{0};
      if (auto slash = entry.display().find_last_of('/');
          !entry.active() && slash != std::string::npos)
      {
        dim_until = slash + 1;
      }

      /* row construction */
      Elements row{ text(label(entry.kind())) | color(Color::RGB(0, 0, 0)) | bgcolor(color_for(entry.kind())) | bold,
                    text(" "),
                    highlighted(entry.display(), match.indices, dim_until)};
      if (entry.active()) { row.push_back(text("*") | color(Color::Green)); }
      row.push_back(text("  "));
      row.push_back(text(entry.extra) | color(classify_text_color(entry.extra)));
      row.push_back(filler());
      row.push_back(text("<" +std::to_string(match.score)+ ">") | color(Color::GrayDark));
      rows.push_back(hbox(std::move(row)) | style);
      /* end row construction */
    }
    if (g_loading != 0) { rows.push_back(text("Loading Entries") | dim); }
    if (rows.empty())   { rows.push_back(text("No matching entries") | dim); }

    Elements search{text("> ") | bold, text(s_query), text(" ") | inverted};
    if (!s_status.empty())
    {
      search.push_back(text("  " + s_status) | color(Color::Yellow));
    }
    return vbox({hbox(std::move(search)), separator(),
        vbox(std::move(rows)) | yframe | flex}) | border;
  });

  /* Keybinds logic */
  auto component = CatchEvent(renderer, [&](Event event) {
    const auto filtered = matches(entries, s_query, history_entries);
    if (s_selected >= filtered.size())
    {
      s_selected = filtered.empty() ? 0 : filtered.size() - 1;
    }
    const Entry* selected_entry = filtered.empty() ? nullptr
                                                   : &entries[filtered[s_selected].index];
    const auto quit = [&] {
      app.ExitLoopClosure()();
      return true;
    };
    if (event == Event::Escape || event == Event::CtrlC || (event == Event::Character("q") && s_query.empty()))
    {
      return quit();
    }
    if (event == Event::Return)
    {
      if (selected_entry == nullptr) { return true; }
      record_history(history_entries, selected_entry->info());
      execute(*selected_entry, active_entries);
      return quit();
    }
    if (event == Event::ArrowUp || event == Event::CtrlK)
    {
      if (s_selected > 0) --s_selected;
      return true;
    }
    if (event == Event::ArrowDown || event == Event::CtrlJ)
    {
      if (s_selected + 1 < filtered.size()) ++s_selected;
      return true;
    }
    if (event == Event::PageUp)
    {
      s_selected = s_selected > 5 ? s_selected - 5 : 0;
      return true;
    }
    if (event == Event::PageDown)
    {
      s_selected = std::min(s_selected + 5, filtered.empty() ? 0 : filtered.size() - 1);
      return true;
    }
    if (event == Event::CtrlY)
    {
      if (selected_entry != nullptr && copy_info(selected_entry->info()))
      {
          s_status = "Copied: " + std::string{selected_entry->info()};
      }
      return true;
    }
    if (event == Event::CtrlT)
    {
      auto theme = toggle_theme();
      selected_color = selection_color();
      s_status = "Swithced to " + theme;
      return true;
    }
    if (event == Event::Backspace || event == Event::CtrlU ||
        event == Event::CtrlW || event.is_character())
    {
      if (event == Event::Backspace && !s_query.empty()) { s_query.pop_back(); }
      else if (event == Event::CtrlU) { s_query.clear(); }
      else if (event == Event::CtrlW) { delete_word(s_query); }
      else if (event.is_character()) { s_query += event.character(); }
      s_selected = 0;
      s_status.clear();
      return true;
    }
    return false;
  });

  app.Loop(component);
}
