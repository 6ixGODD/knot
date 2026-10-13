#include <knot/knot.hpp>
#include <iostream>

int main(int argc, char **argv) {
    if (argc != 2) {
        std::cerr << "Usage: speaker <speaker.json>\n";
        return 2;
    }
    try {
        auto model = knot::Model::compile_file(argv[1]);
        auto result = model.initial();
        auto set = [&](const std::string &id, knot::Answer value) {
            result = model.apply(
                result.state, knot::Event::set_answer(id, std::move(value), result.state.revision));
        };
        set("speaker_type", std::string("active"));
        set("material", std::string("aluminum"));
        set("panel_color", std::string("#315A79"));
        set("power", std::string("200w"));
        set("extras", std::vector<std::string>{"led"});
        auto display = [&] {
            const auto &price = result.snapshot.price;
            if (price.total_minor)
                std::cout << *price.total_minor << '\n';
            else
                std::cout << "unknown\n";
        };
        display();
        set("material", std::string("walnut"));
        display();
        // Save the whole returned State; restore with this same model.
        const auto saved = knot::to_json(result.state);
        result = model.evaluate(knot::state_from_json(saved));
        return 0;
    } catch (const knot::Error &error) {
        std::cerr << error.code << " at " << error.path << ": " << error.what() << '\n';
        return 3;
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 2;
    }
}
