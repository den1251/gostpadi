/* ЛБ2, задача 2: for и break.
 * Факториал с ограничением, чтобы не переполнить long long. */
#include <stdio.h>

#define MAX_N 12

int main(void) {
    int n = 0;
    int i = 0;
    long long f = 1;

    for (;;) {
        printf("n (1..%d): ", MAX_N);
        if (scanf("%d", &n) != 1) {
            printf("нужно число, повтори\n");
            while (getchar() != '\n') {
            }
            continue;
        }
        if (n < 1 || n > MAX_N) {
            printf("n вне диапазона 1..%d\n", MAX_N);
            continue;
        }
        break;
    }
    for (i = 2; i <= n; i++) {
        f = f * i;
    }
    printf("%d! = %lld\n", n, f);
    return 0;
}
