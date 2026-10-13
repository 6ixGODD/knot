#include "bridge.hpp"
#include "../json-isolation/probe.hpp"
#include <stdexcept>
bool p0_cpp_evaluate(const p0_probe* model,const std::string& input) {
    auto result=p0_probe_evaluate(model,reinterpret_cast<const uint8_t*>(input.data()),input.size());
    struct Release { p0_response* result; ~Release(){p0_probe_result_free(result);} } release{&result};
    if(result.status!=0) throw std::runtime_error("P0 backend error");
    const auto value=p0_json_probe::parse(std::string(reinterpret_cast<const char*>(result.json.data),result.json.len));
    if(value.text!="zen"||(value.number!=0&&value.number!=1)) throw std::runtime_error("P0 invalid output");
    return value.number==1;
}
