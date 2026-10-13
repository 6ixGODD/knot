#include <cstdio>
int host_parse();
int bundled_parse();

int main() {
    const int host = host_parse();
    const int bundled = bundled_parse();
    std::printf("host_result=%d bundled_result=%d expected_host=53 expected_bundled=54\n",
                host, bundled);
    return host == 53 && bundled == 54 ? 0 : 1;
}
