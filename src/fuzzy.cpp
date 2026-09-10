#include "fuzzy.hpp"
#include "util.hpp"
#include <cctype>
#include <ranges>
#include <algorithm>
#include <vector>

struct Range
{
	size_t begin;
	size_t end; // not inclusive
};

static bool
substring_equal_at(std::string_view text, size_t position, std::string_view substr)
{
	if (position + substr.size() > text.size()) return false;
	for (size_t i = 0; i < substr.size(); ++i)
	{
		if (std::tolower(text[position + i]) != std::tolower(substr[i]))
			return false;
	}
	return true;
}

static bool
overlaps(const std::vector<Range> &ranges, size_t begin, size_t end)
{
	return std::ranges::any_of(ranges, [&](const Range &range) {
			return begin < range.end && range.begin < end;
	});
}

// Make sure that the passed query is trimmed
FuzzyMatch
fuzzy_match(std::string_view text, std::string_view query, std::span<size_t> matched_indices)
{
	if (query.empty()) return {.matched = true};

	std::vector<Range> ranges;
	size_t matched_chars{0};
	const auto trm_query = trim(query);
	for (size_t begin = 0; begin < trm_query.size();)
	{
		size_t end;// this get's populated here       v
		const auto part = get_word(trm_query, begin, end);
		bool found = false;
		for (size_t position = 0; position + part.size() <= text.size(); ++position)
		{
			if (substring_equal_at(text, position, part) && !overlaps(ranges, position, position + part.size()))
			{
				ranges.push_back({position, position + part.size()});
				matched_chars += part.size();
				found = true;
				break;
			}
		}
		if (!found) return {};
		begin = end;
	}

	std::ranges::sort(ranges, {}, &Range::begin);
	size_t count{0};
	for (const auto &range : ranges)
	{
		for (auto index = range.begin; index < range.end && count < matched_indices.size(); ++index)
			matched_indices[count++] = index;
	}

	return {.matched = true,
			.score = text.empty() ? 0 : (int)(matched_chars * 100 / text.size()),
			.indices = matched_indices.first(count)};
}
