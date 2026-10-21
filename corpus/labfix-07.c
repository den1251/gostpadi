/* ЛБ2, задача 4: вложенные циклы.
 * Простые числа до n, break во внутреннем цикле. */
#include <stdio.h>

#define N_MAX 100

int main(void) {
    int n = 0;
    int i = 0;
    int j = 0;
    int is_prime = 0;

    while (1) {
        printf("n (2..%d): ", N_MAX);
        if (scanf("%d", &n) == 1 && n >= 2 && n <= N_MAX) {
            break;
        }
        printf("нужно число 2..%d, повтори\n", N_MAX);
        while (getchar() != '\n') {
        }
    }
    printf("простые числа до %d:\n", n);
    for (i = 2; i <= n; i++) {
        is_prime = 1;
        for (j = 2; j * j <= i; j++) {
            if (i % j == 0) {
                is_prime = 0;
                break;
            }
        }
        if (is_prime) {
            printf("%d ", i);
        }
    }
    printf("\n");
    return 0;
}
