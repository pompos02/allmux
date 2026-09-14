#pragma once

#include <chrono>
#include <filesystem>
#include <format>
#include <fstream>
#include <iomanip>
#include <mutex>
#include <sstream>
#include <string>
#include <string_view>
#include <utility>

std::filesystem::path log_file();

#define WriteLog(...)                                                          \
  ::logger::log_message(__FILE__, __LINE__, __func__, __VA_ARGS__)

namespace logger
{

inline std::mutex log_mutex;

inline std::string
timestamp()
{
  const auto now = std::chrono::system_clock::now();
  const auto time = std::chrono::system_clock::to_time_t(now);

  std::tm tm{};
  localtime_r(&time, &tm);

  std::ostringstream out;
  out << std::put_time(&tm, "%H:%M:%S");
  return out.str();
}

template <typename... Args>
void
log_message(std::string_view file, int line, std::string_view function,
            std::format_string<Args...> fmt, Args &&...args)
{
  std::lock_guard lock(log_mutex);

  std::ofstream out{log_file(), std::ios::app};
  if (!out) return;

  out << timestamp() << " [" << file << ':' << line << "]@" << function << " | "
      << std::format(fmt, std::forward<Args>(args)...) << '\n';

  out.flush();
}

} // namespace logger
