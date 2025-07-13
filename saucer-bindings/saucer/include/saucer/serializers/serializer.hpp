#pragma once

#include "data.hpp"

#include "args/args.hpp"
#include "../executor.hpp"

#include <concepts>
#include <functional>

#include <string>
#include <memory>
#include <future>

#include <fmt/args.h>

namespace saucer
{
    struct serializer
    {
        using parse_result = message_data;
        using executor     = saucer::executor<std::string>;
        using args         = fmt::dynamic_format_arg_store<fmt::format_context>;

      public:
        using resolver = std::move_only_function<void(std::unique_ptr<result_data>)>;
        using function = std::move_only_function<void(std::unique_ptr<function_data>, executor)>;

      public:
        virtual ~serializer() = default;

      public:
        [[nodiscard]] virtual std::string script() const        = 0;
        [[nodiscard]] virtual std::string js_serializer() const = 0;

      public:
        [[nodiscard]] virtual parse_result parse(const std::string &) const = 0;
    };

    template <class T>
    concept Serializer = requires {
        requires std::movable<T>;
        requires std::derived_from<T, serializer>;
        { //
            T::serialize(std::function<int()>{})
        } -> std::convertible_to<serializer::function>;
        { //
            T::serialize(std::function<void(executor<int>)>{})
        } -> std::convertible_to<serializer::function>;
        { //
            T::serialize_args(10, 15, 20)
        } -> std::convertible_to<serializer::args>;
        { //
            T::serialize_args(make_args(10, 15, 20))
        } -> std::convertible_to<serializer::args>;
        { //
            T::resolve(std::declval<std::promise<int>>())
        } -> std::convertible_to<serializer::resolver>;
    };
} // namespace saucer
