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
		if (std::tolower(static_cast<unsigned char>(text[position + i])) !=
		    std::tolower(static_cast<unsigned char>(substr[i])))
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

FuzzyMatch
fuzzy_match(std::string_view text, std::string_view query)
{
	query = trim(query);
	if (query.empty()) return {};

	std::vector<Range> ranges;
	size_t matched_chars{0};
	for (size_t begin = 0; begin < query.size();)
	{
		for (; begin < query.size() && std::isspace(static_cast<unsigned char>(query[begin])); ++begin) { }
		size_t end = begin;
		for (; end < query.size() && !std::isspace(static_cast<unsigned char>(query[end])); ++end) { }
		const auto part = query.substr(begin, end - begin);
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
	std::vector<size_t> indices;
	indices.reserve(matched_chars);
	for (const auto &range : ranges)
	{
		for (auto index = range.begin; index < range.end; ++index)
			indices.push_back(index);
	}

	const int score = text.empty() ? 0 : static_cast<int>(matched_chars * 100 / text.size());
	return {.matched = true, .score = score, .indices = std::move(indices)};
}
