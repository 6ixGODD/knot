#include "bridge.hpp"
#include <atomic>
#include <iostream>
#include <thread>
#include <vector>
int main() {
    p0_probe* model=nullptr;
    auto result=p0_probe_create(&model);
    const bool failed=result.status!=0||model==nullptr;
    p0_probe_result_free(&result);
    if(failed) return 1;
    std::atomic<int> failures{0};
    std::vector<std::thread> threads;
    for(int i=0;i<8;i++) threads.emplace_back([&,i]{
        try {
            for(int n=0;n<100;n++) {
                const bool actual=p0_cpp_evaluate(model,i%2?"{\"value\":7}":"{\"value\":8}");
                if(actual!=(i%2!=0)) failures++;
            }
        } catch(...) { failures++; }
    });
    for(auto& thread:threads) thread.join();
    bool rejected=false;
    try {p0_cpp_evaluate(model,"{");}catch(const std::exception&){rejected=true;}
    if(!rejected) failures++;
    p0_probe_destroy(model);
    std::cout<<"native C++ failures="<<failures<<'\n';
    return failures?1:0;
}
