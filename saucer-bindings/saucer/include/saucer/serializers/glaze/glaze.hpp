#pragma once

#include "../generic/generic.hpp"

#include <glaze/glaze.hpp>

namespace saucer::serializers::glaze
{
    struct function_data : saucer::function_data
    {
        glz::raw_json params;
    };

    struct result_data : saucer::result_data
    {
        glz::raw_json result;
    };

    class interface
    {
        template <typename T>
        using result = std::expected<T, std::string>;

      public:
        template <typename T>
        static result<T> parse(const std::string &);

      public:
        template <typename T>
        static result<T> parse(const result_data &);

        template <typename T>
        static result<T> parse(const function_data &);

      public:
        template <typename T>
        static std::string serialize(T &&);
    };

    struct serializer : generic::serializer<function_data, result_data, interface>
    {
        ~serializer() override;

      public:
        [[nodiscard]] std::string script() const override;
        [[nodiscard]] std::string js_serializer() const override;

      public:
        [[nodiscard]] parse_result parse(const std::string &) const override;
    };
} // namespace saucer::serializers::glaze

#include "glaze.inl"
