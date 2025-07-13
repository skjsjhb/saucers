#include "serializers/rflpp/rflpp.hpp"

#include <expected>

namespace rfl
{
    using namespace saucer::serializers::rflpp;

    template <>
    struct Reflector<function_data>
    {
        struct ReflType
        {
            rfl::Rename<"saucer:call", bool> tag;
            std::uint64_t id;
            std::string name;
            rfl::Generic params;
        };

        static function_data to(const ReflType &v) noexcept
        {
            return {{v.id, v.name}, v.params};
        }
    };

    template <>
    struct Reflector<result_data>
    {
        struct ReflType
        {
            rfl::Rename<"saucer:resolve", bool> tag;
            std::uint64_t id;
            rfl::Generic result;
        };

        static result_data to(const ReflType &v) noexcept
        {
            return {{v.id}, v.result};
        }
    };
} // namespace rfl

namespace saucer::serializers::rflpp
{
    serializer::~serializer() = default;

    std::string serializer::script() const
    {
        return {};
    }

    std::string serializer::js_serializer() const
    {
        return "JSON.stringify";
    }

    template <typename T>
    std::optional<T> parse_as(const std::string &buffer)
    {
        auto result = rfl::json::read<T>(buffer);

        if (!result)
        {
            return std::nullopt;
        }

        return result.value();
    }

    serializer::parse_result serializer::parse(const std::string &data) const
    {
        if (auto res = parse_as<function_data>(data); res.has_value())
        {
            return std::make_unique<function_data>(res.value());
        }

        if (auto res = parse_as<result_data>(data); res.has_value())
        {
            return std::make_unique<result_data>(res.value());
        }

        return std::monostate{};
    }
} // namespace saucer::serializers::rflpp
