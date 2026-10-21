/* ЛБ1, задача 2: разветвляющийся алгоритм.
 * Максимум из трёх чисел: каскад if/else if и вложенный if. */
#include <stdio.h>

int main(void) {
    int a = 0;
    int b = 0;
    int c = 0;
    int max = 0;

    printf("три целых числа a b c: ");
    scanf("%d %d %d", &a, &b, &c);
    if (a >= b && a >= c) {
        max = a;
    } else if (b >= a && b >= c) {
        max = b;
    } else {
        max = c;
    }
    printf("каскад: max = %d\n", max);
    if (a >= b) {
        if (a >= c) {
            max = a;
        } else {
            max = c;
        }
    } else {
        if (b >= c) {
            max = b;
        } else {
            max = c;
        }
    }
    printf("вложенный if: max = %d\n", max);
    return 0;
}
