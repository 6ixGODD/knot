#include "probe.h"
#include <string.h>
#include <stdio.h>
static int check(p0_probe* model, const char* input, int status, const char* expected) {
    p0_response result=p0_probe_evaluate(model,(const uint8_t*)input,strlen(input));
    int failed=result.status!=status;
    if(expected) failed|=result.json.len!=strlen(expected)||memcmp(result.json.data,expected,strlen(expected))!=0;
    p0_probe_result_free(&result);
    p0_probe_result_free(&result);
    return failed||result.json.data!=NULL||result.json.len!=0;
}
int main(void) {
    p0_probe* model=NULL;
    p0_response created=p0_probe_create(&model);
    int failed=created.status!=0||model==NULL;
    p0_probe_result_free(&created);
    if(failed) return 1;
    for(int i=0;i<100;i++) {
        failed|=check(model,"{\"value\":7}",0,"{\"text\":\"zen\",\"value\":1}");
        failed|=check(model,"{\"value\":8}",0,"{\"text\":\"zen\",\"value\":0}");
        failed|=check(model,"{}",0,"{\"text\":\"zen\",\"value\":0}");
    }
    failed|=check(model,"{",1002,NULL);
    failed|=check(model,"{\"value\":9007199254740991.1}",1003,NULL);
    p0_probe_destroy(model);
    printf("native C probe failed=%d\n",failed);
    return failed?1:0;
}
